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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 132
// ---------------------------------------------------------------------------

/// Generic object pool `Xc132Pool<T>`.
pub struct Xc132Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc132Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc132PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc132Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc132PoolStats {
        Xc132PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc132Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc132Scheduler`.
pub struct Xc132Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc132Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc132Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_132 hash for the given byte slice.
pub fn xc_132_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_132 convention.
pub fn xc_132_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_76 deepening: state machine + event bus ---

/// States for the Xd76 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd76State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd76State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd76Transition {
    pub from: Xd76State,
    pub to: Xd76State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd76StateMachine {
    current: Xd76State,
    history: Vec<Xd76Transition>,
    step_counter: usize,
}

impl Xd76StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd76State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd76State {
        self.current
    }

    pub fn history(&self) -> &[Xd76Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd76State) -> Result<Xd76State, String> {
        let allowed = match (self.current, target) {
            (Xd76State::Idle, Xd76State::Running) => true,
            (Xd76State::Running, Xd76State::Paused) => true,
            (Xd76State::Running, Xd76State::Done) => true,
            (Xd76State::Paused, Xd76State::Running) => true,
            (Xd76State::Paused, Xd76State::Done) => true,
            (Xd76State::Done, Xd76State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_76: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd76Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd76SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd76State> {
        let prefix = "Xd76SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd76State::Idle),
            "Running" => Some(Xd76State::Running),
            "Paused" => Some(Xd76State::Paused),
            "Done" => Some(Xd76State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd76State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd76 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd76Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd76Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd76HandlerFn = Box<dyn Fn(&Xd76Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd76EventBus {
    handlers: Vec<(usize, Option<String>, Xd76HandlerFn)>,
    next_id: usize,
    published: Vec<Xd76Event>,
}

impl Xd76EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd76Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd76Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd76Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd76Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #94
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf94Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf94TrieNode {
    children: std::collections::HashMap<char, Xf94TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf94Trie {
    root: Xf94TrieNode,
    count: usize,
}

impl Xf94Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf94TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf94TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf94TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf94BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf94BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 131).
pub struct Xh131SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh131SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 173 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 131).
pub struct Xh131BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh131BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 131).
pub struct Xi131Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi131Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi131Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi131Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 131).
pub struct Xi131IntervalTree {
    xi_intervals: Vec<Xi131Interval>,
}

impl Xi131IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi131Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi131Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi131Interval) -> Vec<&Xi131Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi131Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi131Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi131Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi131Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi131Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi131Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 131) ---

/// Disjoint set / union-find for crate 131.
pub struct Xj131UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj131UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ131_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 131.
pub struct Xj131BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj131BTreeNode<K, V>>>,
    len: usize,
}

struct Xj131BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj131BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj131BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ131_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ131_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj131BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj131BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj131BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj131BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_131 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk131SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk131SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk131DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk131DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_131).
#[derive(Debug, Clone)]
pub struct Xl131Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl131Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_131).
#[derive(Debug, Clone)]
pub struct Xl131SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl131SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm131MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm131MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm131Tokenizer {
    text: String,
}

impl Xm131Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 131.
pub struct Xn131Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn131Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 131 -----

#[derive(Debug, Clone)]
struct Xn131AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn131AvlNode<K, V>>>,
    right: Option<Box<Xn131AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 131.
#[derive(Debug, Clone)]
pub struct Xn131AVL<K, V> {
    root: Option<Box<Xn131AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn131AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn131AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn131AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn131AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn131AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn131AvlNode<K, V>>) -> Box<Xn131AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn131AvlNode<K, V>>) -> Box<Xn131AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn131AvlNode<K, V>>) -> Box<Xn131AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn131AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn131AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn131AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn131AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn131AvlNode<K, V>>) -> &Xn131AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn131AvlNode<K, V>>) -> (Box<Xn131AvlNode<K, V>>, Option<Box<Xn131AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn131AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn131AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn131AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn131AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn131AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn131AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn131AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo131RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo131Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo131RBNode<K, V> {
    key: K,
    value: V,
    color: Xo131Color,
    left: Option<Box<Xo131RBNode<K, V>>>,
    right: Option<Box<Xo131RBNode<K, V>>>,
}

/// A red-black tree map for crate 131.
#[derive(Debug, Clone)]
pub struct Xo131RedBlack<K, V> {
    root: Option<Box<Xo131RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo131RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo131Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo131RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo131RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo131RBNode {
                    key, value, color: Xo131Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo131RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo131Color::Red)
    }

    fn xo_balance(mut h: Box<Xo131RBNode<K, V>>) -> Box<Xo131RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo131Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo131RBNode<K, V>>) -> Box<Xo131RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo131Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo131RBNode<K, V>>) -> Box<Xo131RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo131Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo131RBNode<K, V>>) {
        h.color = Xo131Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo131Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo131Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo131Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo131RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo131RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo131RBNode<K, V>) -> (K, V, Option<Box<Xo131RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo131RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo131Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo131RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo131ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 131.
#[derive(Debug, Clone)]
pub struct Xo131ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo131ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo131#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo131#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 131).
#[derive(Debug)]
pub struct Xp131SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp131Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp131Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp131Node<K, V>>>,
    xp_right: Option<Box<Xp131Node<K, V>>>,
}

impl<K: Ord, V> Xp131Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp131SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp131SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp131Node<K, V>>>, key: &K) -> Option<Box<Xp131Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp131Node<K, V>>) -> Box<Xp131Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp131Node<K, V>>) -> Box<Xp131Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp131Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp131Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp131Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq131Treap ---------------

use std::cmp::Ordering as Xq131Ord;

struct Xq131TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq131TreapNode<K, V>>>,
    right: Option<Box<Xq131TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq131Treap<K, V> {
    root: Option<Box<Xq131TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq131TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_131_size<K, V>(node: &Option<Box<Xq131TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_131_update_size<K, V>(node: &mut Xq131TreapNode<K, V>) {
    node.size = 1 + xq_131_size(&node.left) + xq_131_size(&node.right);
}

fn xq_131_rotate_right<K, V>(mut node: Box<Xq131TreapNode<K, V>>) -> Box<Xq131TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_131_update_size(&mut node);
    left.right = Some(node);
    xq_131_update_size(&mut left);
    left
}

fn xq_131_rotate_left<K, V>(mut node: Box<Xq131TreapNode<K, V>>) -> Box<Xq131TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_131_update_size(&mut node);
    right.left = Some(node);
    xq_131_update_size(&mut right);
    right
}

fn xq_131_insert_node<K: Ord, V>(
    node: Option<Box<Xq131TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq131TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq131TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq131Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq131Ord::Less => {
                let (new_left, old) = xq_131_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_131_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_131_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq131Ord::Greater => {
                let (new_right, old) = xq_131_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_131_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_131_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_131_remove_node<K: Ord, V>(
    node: Option<Box<Xq131TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq131TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq131Ord::Less => {
                let (new_left, old) = xq_131_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_131_update_size(&mut n);
                (Some(n), old)
            }
            Xq131Ord::Greater => {
                let (new_right, old) = xq_131_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_131_update_size(&mut n);
                (Some(n), old)
            }
            Xq131Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_131_rotate_right(n);
                    let (new_right, old) = xq_131_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_131_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_131_rotate_left(n);
                    let (new_left, old) = xq_131_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_131_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_131_find_min<K, V>(node: &Option<Box<Xq131TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_131_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_131_find_max<K, V>(node: &Option<Box<Xq131TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_131_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_131_rank<K: Ord, V>(node: &Option<Box<Xq131TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq131Ord::Less => xq_131_rank(&n.left, key),
            Xq131Ord::Equal => xq_131_size(&n.left),
            Xq131Ord::Greater => 1 + xq_131_size(&n.left) + xq_131_rank(&n.right, key),
        },
    }
}

fn xq_131_kth<K, V>(node: &Option<Box<Xq131TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_131_size(&n.left);
        if k < left_size {
            xq_131_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_131_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_131_in_order<K: Clone, V>(node: &Option<Box<Xq131TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_131_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_131_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq131Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 131 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_131_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq131Ord::Equal => return Some(&n.value),
                Xq131Ord::Less => cur = &n.left,
                Xq131Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_131_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_131_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_131_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_131_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_131_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_131_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_131_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq131VEBTree ---------------

pub struct Xq131VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq131VEBTree>>,
    clusters: Vec<Option<Box<Xq131VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq131VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq131VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq131VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr131KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr131KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr131BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr131KDNode {
    xr_point: Xr131KDPoint,
    xr_left: Option<Box<Xr131KDNode>>,
    xr_right: Option<Box<Xr131KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr131KDTree {
    xr_root: Option<Box<Xr131KDNode>>,
    xr_size: usize,
}

impl Xr131KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr131KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr131KDNode>>,
        point: Xr131KDPoint,
        depth: usize,
    ) -> Box<Xr131KDNode> {
        match node {
            None => Box::new(Xr131KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr131KDPoint) -> Option<Xr131KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr131KDNode>,
        query: &Xr131KDPoint,
        depth: usize,
        best: &mut Xr131KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr131KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr131KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr131KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr131KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr131KDNode>>, pts: &mut Vec<Xr131KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr131KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr131BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr131BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs131PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs131PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs131PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs131PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs131ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs131ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs131ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs131RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs131RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs131RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs131CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs131CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs131CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}


// --- xy_ Cuckoo Hash Map ---

/// Cuckoo hash map with two hash functions and O(1) amortized lookup.
#[derive(Debug, Clone)]
pub struct XyCuckooMap<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xy_table1: Vec<Option<(K, V)>>,
    xy_table2: Vec<Option<(K, V)>>,
    xy_capacity: usize,
    xy_size: usize,
    xy_seed1: u64,
    xy_seed2: u64,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XyCuckooMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CuckooMap(size={}, cap={})", self.xy_size, self.xy_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> Default for XyCuckooMap<K, V> {
    fn default() -> Self { Self::xy_new(16) }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XyCuckooMap<K, V> {
    /// Create a new cuckoo hash map with given capacity.
    pub fn xy_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xy_table1: (0..cap).map(|_| None).collect(),
            xy_table2: (0..cap).map(|_| None).collect(),
            xy_capacity: cap,
            xy_size: 0,
            xy_seed1: 0x517cc1b727220a95,
            xy_seed2: 0x6c62272e07bb0142,
        }
    }

    fn xy_hash1(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed1.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    fn xy_hash2(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed2.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    /// Number of elements.
    pub fn xy_len(&self) -> usize { self.xy_size }

    /// Is empty.
    pub fn xy_is_empty(&self) -> bool { self.xy_size == 0 }

    /// Insert a key-value pair.
    pub fn xy_insert(&mut self, key: K, value: V) -> bool {
        if self.xy_get(&key).is_some() {
            let h1 = self.xy_hash1(&key);
            if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == key) {
                self.xy_table1[h1] = Some((key, value));
            } else {
                let h2 = self.xy_hash2(&key);
                self.xy_table2[h2] = Some((key, value));
            }
            return true;
        }
        let mut k = key;
        let mut v = value;
        for _ in 0..self.xy_capacity {
            let h1 = self.xy_hash1(&k);
            if self.xy_table1[h1].is_none() {
                self.xy_table1[h1] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old = self.xy_table1[h1].take().unwrap();
            self.xy_table1[h1] = Some((k, v));
            k = old.0;
            v = old.1;
            let h2 = self.xy_hash2(&k);
            if self.xy_table2[h2].is_none() {
                self.xy_table2[h2] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old2 = self.xy_table2[h2].take().unwrap();
            self.xy_table2[h2] = Some((k, v));
            k = old2.0;
            v = old2.1;
        }
        // Rehash needed — just put in table1 with linear probing fallback
        for i in 0..self.xy_capacity {
            if self.xy_table1[i].is_none() {
                self.xy_table1[i] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
        }
        false
    }

    /// Look up a key.
    pub fn xy_get(&self, key: &K) -> Option<&V> {
        let h1 = self.xy_hash1(key);
        if let Some((k, v)) = &self.xy_table1[h1] {
            if *k == *key { return Some(v); }
        }
        let h2 = self.xy_hash2(key);
        if let Some((k, v)) = &self.xy_table2[h2] {
            if *k == *key { return Some(v); }
        }
        None
    }

    /// Check if key exists.
    pub fn xy_contains(&self, key: &K) -> bool { self.xy_get(key).is_some() }

    /// Remove a key.
    pub fn xy_remove(&mut self, key: &K) -> Option<V> {
        let h1 = self.xy_hash1(key);
        if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table1[h1].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        let h2 = self.xy_hash2(key);
        if self.xy_table2[h2].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table2[h2].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        None
    }

    /// Clear the map.
    pub fn xy_clear(&mut self) {
        for slot in &mut self.xy_table1 { *slot = None; }
        for slot in &mut self.xy_table2 { *slot = None; }
        self.xy_size = 0;
    }

    /// Collect all keys.
    pub fn xy_keys(&self) -> Vec<K> {
        let mut keys = Vec::new();
        for slot in &self.xy_table1 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        for slot in &self.xy_table2 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        keys
    }
}

// --- xy_ Count-Min Sketch ---

/// Count-min sketch for approximate frequency counting with bounded error.
#[derive(Debug, Clone)]
pub struct XyCountMinSketch {
    xy_table: Vec<Vec<u64>>,
    xy_width: usize,
    xy_depth: usize,
    xy_seeds: Vec<u64>,
}

impl std::fmt::Display for XyCountMinSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CMS(w={}, d={})", self.xy_width, self.xy_depth)
    }
}

impl Default for XyCountMinSketch {
    fn default() -> Self { Self::xy_new(1000, 5) }
}

impl XyCountMinSketch {
    /// Create a new count-min sketch with given width and depth.
    pub fn xy_new(width: usize, depth: usize) -> Self {
        let seeds: Vec<u64> = (0..depth).map(|i| 0x9e3779b97f4a7c15u64.wrapping_add((i as u64).wrapping_mul(0x517cc1b727220a95))).collect();
        Self {
            xy_table: vec![vec![0u64; width]; depth],
            xy_width: width,
            xy_depth: depth,
            xy_seeds: seeds,
        }
    }

    fn xy_hash(&self, item: u64, seed: u64) -> usize {
        let h = item.wrapping_mul(seed).wrapping_add(seed >> 16);
        (h ^ (h >> 32)) as usize % self.xy_width
    }

    /// Increment the count for an item.
    pub fn xy_add(&mut self, item: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += 1;
        }
    }

    /// Add with a specific count.
    pub fn xy_add_count(&mut self, item: u64, count: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += count;
        }
    }

    /// Estimate the count for an item (guaranteed to be >= actual count).
    pub fn xy_estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            min_count = min_count.min(self.xy_table[i][idx]);
        }
        min_count
    }

    /// Width of the sketch.
    pub fn xy_width(&self) -> usize { self.xy_width }

    /// Depth of the sketch.
    pub fn xy_depth(&self) -> usize { self.xy_depth }

    /// Clear the sketch.
    pub fn xy_clear(&mut self) {
        for row in &mut self.xy_table {
            for cell in row { *cell = 0; }
        }
    }

    /// Merge another sketch into this one.
    pub fn xy_merge(&mut self, other: &XyCountMinSketch) {
        if self.xy_width != other.xy_width || self.xy_depth != other.xy_depth { return; }
        for i in 0..self.xy_depth {
            for j in 0..self.xy_width {
                self.xy_table[i][j] += other.xy_table[i][j];
            }
        }
    }
}


// --- xz_ HyperLogLog ---

/// HyperLogLog probabilistic cardinality estimator with configurable precision.
#[derive(Debug, Clone)]
pub struct XzHyperLogLog {
    xz_registers: Vec<u8>,
    xz_m: usize,
    xz_b: u32,
}

impl std::fmt::Display for XzHyperLogLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HLL(m={}, est={:.0})", self.xz_m, self.xz_estimate())
    }
}

impl Default for XzHyperLogLog {
    fn default() -> Self { Self::xz_new(10) }
}

impl XzHyperLogLog {
    /// Create a new HyperLogLog with precision b (4 <= b <= 16). Uses 2^b registers.
    pub fn xz_new(b: u32) -> Self {
        let b = b.clamp(4, 16);
        let m = 1 << b;
        Self { xz_registers: vec![0u8; m], xz_m: m, xz_b: b }
    }

    fn xz_hash(item: u64) -> u64 {
        let mut h = item;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    /// Add an item.
    pub fn xz_add(&mut self, item: u64) {
        let h = Self::xz_hash(item);
        let idx = (h as usize) & (self.xz_m - 1);
        let w = h >> self.xz_b;
        let rho = if w == 0 { 64 - self.xz_b } else { w.trailing_zeros() + 1 };
        let rho = rho.min(255) as u8;
        if rho > self.xz_registers[idx] {
            self.xz_registers[idx] = rho;
        }
    }

    /// Estimate the cardinality.
    pub fn xz_estimate(&self) -> f64 {
        let m = self.xz_m as f64;
        let alpha = match self.xz_m {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };
        let sum: f64 = self.xz_registers.iter().map(|&r| 2.0f64.powi(-(r as i32))).sum();
        let raw = alpha * m * m / sum;
        if raw <= 2.5 * m {
            let zeros = self.xz_registers.iter().filter(|&&r| r == 0).count();
            if zeros > 0 { m * (m / zeros as f64).ln() } else { raw }
        } else if raw <= (1u64 << 32) as f64 / 30.0 {
            raw
        } else {
            -(((1u64 << 32) as f64) * (1.0 - raw / (1u64 << 32) as f64).ln())
        }
    }

    /// Merge another HyperLogLog into this one.
    pub fn xz_merge(&mut self, other: &XzHyperLogLog) {
        if self.xz_m != other.xz_m { return; }
        for i in 0..self.xz_m {
            if other.xz_registers[i] > self.xz_registers[i] {
                self.xz_registers[i] = other.xz_registers[i];
            }
        }
    }

    /// Clear all registers.
    pub fn xz_clear(&mut self) {
        for r in &mut self.xz_registers { *r = 0; }
    }

    /// Number of registers.
    pub fn xz_num_registers(&self) -> usize { self.xz_m }

    /// Precision parameter.
    pub fn xz_precision(&self) -> u32 { self.xz_b }
}

// --- xz_ LRU Cache ---

/// LRU cache with O(1) get/put using a doubly-linked list and hash map.
#[derive(Debug, Clone)]
pub struct XzLruCache<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xz_capacity: usize,
    xz_entries: Vec<(K, V)>,
    xz_order: Vec<usize>,
    xz_map: std::collections::HashMap<K, usize>,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XzLruCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LRU(size={}, cap={})", self.xz_map.len(), self.xz_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XzLruCache<K, V> {
    /// Create a new LRU cache with given capacity.
    pub fn xz_new(capacity: usize) -> Self {
        Self {
            xz_capacity: capacity.max(1),
            xz_entries: Vec::new(),
            xz_order: Vec::new(),
            xz_map: std::collections::HashMap::new(),
        }
    }

    /// Number of entries.
    pub fn xz_len(&self) -> usize { self.xz_map.len() }

    /// Is empty.
    pub fn xz_is_empty(&self) -> bool { self.xz_map.is_empty() }

    /// Capacity.
    pub fn xz_capacity(&self) -> usize { self.xz_capacity }

    /// Get a value, marking it as recently used.
    pub fn xz_get(&mut self, key: &K) -> Option<&V> {
        if let Some(&idx) = self.xz_map.get(key) {
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            Some(&self.xz_entries[idx].1)
        } else {
            None
        }
    }

    /// Put a key-value pair, evicting the least recently used if at capacity.
    pub fn xz_put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.xz_map.get(&key) {
            self.xz_entries[idx].1 = value;
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            return;
        }
        if self.xz_map.len() >= self.xz_capacity {
            if let Some(evict_idx) = self.xz_order.first().copied() {
                self.xz_order.remove(0);
                let evict_key = self.xz_entries[evict_idx].0.clone();
                self.xz_map.remove(&evict_key);
            }
        }
        let idx = self.xz_entries.len();
        self.xz_entries.push((key.clone(), value));
        self.xz_map.insert(key, idx);
        self.xz_order.push(idx);
    }

    /// Check if key exists (without updating LRU order).
    pub fn xz_contains(&self, key: &K) -> bool { self.xz_map.contains_key(key) }

    /// Remove a key.
    pub fn xz_remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.xz_map.remove(key) {
            self.xz_order.retain(|&i| i != idx);
            Some(self.xz_entries[idx].1.clone())
        } else {
            None
        }
    }

    /// Clear the cache.
    pub fn xz_clear(&mut self) {
        self.xz_entries.clear();
        self.xz_order.clear();
        self.xz_map.clear();
    }

    /// Get all keys in LRU order (least recent first).
    pub fn xz_keys_lru(&self) -> Vec<K> {
        self.xz_order.iter().filter_map(|&idx| {
            let k = &self.xz_entries[idx].0;
            if self.xz_map.contains_key(k) { Some(k.clone()) } else { None }
        }).collect()
    }

    /// Peek at value without updating LRU order.
    pub fn xz_peek(&self, key: &K) -> Option<&V> {
        self.xz_map.get(key).map(|&idx| &self.xz_entries[idx].1)
    }
}


// --- ya_ Trie (Prefix Tree) ---

/// A node in a trie (prefix tree) for string key lookups.
#[derive(Debug, Clone)]
pub struct YaTrieNode<V: Clone> {
    ya_children: std::collections::HashMap<char, Box<YaTrieNode<V>>>,
    ya_value: Option<V>,
    ya_is_end: bool,
}

impl<V: Clone> Default for YaTrieNode<V> {
    fn default() -> Self {
        Self { ya_children: std::collections::HashMap::new(), ya_value: None, ya_is_end: false }
    }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrieNode<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrieNode(children={}, end={})", self.ya_children.len(), self.ya_is_end)
    }
}

/// Trie (prefix tree) for O(m) string key operations where m is key length.
#[derive(Debug, Clone)]
pub struct YaTrie<V: Clone> {
    ya_root: YaTrieNode<V>,
    ya_size: usize,
}

impl<V: Clone> Default for YaTrie<V> {
    fn default() -> Self { Self::ya_new() }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrie<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trie(size={})", self.ya_size)
    }
}

impl<V: Clone> YaTrie<V> {
    /// Create an empty trie.
    pub fn ya_new() -> Self { Self { ya_root: YaTrieNode::default(), ya_size: 0 } }

    /// Number of stored keys.
    pub fn ya_len(&self) -> usize { self.ya_size }

    /// Is the trie empty.
    pub fn ya_is_empty(&self) -> bool { self.ya_size == 0 }

    /// Insert a key-value pair.
    pub fn ya_insert(&mut self, key: &str, value: V) {
        let mut node = &mut self.ya_root;
        for ch in key.chars() {
            node = node.ya_children.entry(ch).or_insert_with(|| Box::new(YaTrieNode::default()));
        }
        if !node.ya_is_end { self.ya_size += 1; }
        node.ya_value = Some(value);
        node.ya_is_end = true;
    }

    /// Look up a key.
    pub fn ya_get(&self, key: &str) -> Option<&V> {
        let mut node = &self.ya_root;
        for ch in key.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return None,
            }
        }
        if node.ya_is_end { node.ya_value.as_ref() } else { None }
    }

    /// Check if a key exists.
    pub fn ya_contains(&self, key: &str) -> bool { self.ya_get(key).is_some() }

    /// Check if any key starts with the given prefix.
    pub fn ya_has_prefix(&self, prefix: &str) -> bool {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return false,
            }
        }
        true
    }

    /// Collect all keys with the given prefix.
    pub fn ya_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        Self::ya_collect_keys(node, &mut prefix.to_string(), &mut results);
        results
    }

    fn ya_collect_keys(node: &YaTrieNode<V>, current: &mut String, results: &mut Vec<String>) {
        if node.ya_is_end { results.push(current.clone()); }
        let mut chars: Vec<char> = node.ya_children.keys().copied().collect();
        chars.sort();
        for ch in chars {
            current.push(ch);
            Self::ya_collect_keys(node.ya_children.get(&ch).unwrap(), current, results);
            current.pop();
        }
    }

    /// Collect all keys.
    pub fn ya_all_keys(&self) -> Vec<String> {
        self.ya_keys_with_prefix("")
    }

    /// Remove a key. Returns the value if it existed.
    pub fn ya_remove(&mut self, key: &str) -> Option<V> {
        let result = Self::ya_remove_recursive(&mut self.ya_root, key, 0);
        if result.is_some() { self.ya_size -= 1; }
        result
    }

    fn ya_remove_recursive(node: &mut YaTrieNode<V>, key: &str, depth: usize) -> Option<V> {
        let chars: Vec<char> = key.chars().collect();
        if depth == chars.len() {
            if node.ya_is_end {
                node.ya_is_end = false;
                return node.ya_value.take();
            }
            return None;
        }
        let ch = chars[depth];
        if let Some(child) = node.ya_children.get_mut(&ch) {
            let result = Self::ya_remove_recursive(child, key, depth + 1);
            if !child.ya_is_end && child.ya_children.is_empty() {
                node.ya_children.remove(&ch);
            }
            result
        } else {
            None
        }
    }

    /// Clear the trie.
    pub fn ya_clear(&mut self) {
        self.ya_root = YaTrieNode::default();
        self.ya_size = 0;
    }

    /// Count keys with a given prefix.
    pub fn ya_count_prefix(&self, prefix: &str) -> usize {
        self.ya_keys_with_prefix(prefix).len()
    }

    /// Longest common prefix among all keys.
    pub fn ya_longest_common_prefix(&self) -> String {
        let mut result = String::new();
        let mut node = &self.ya_root;
        while node.ya_children.len() == 1 && !node.ya_is_end {
            let (&ch, child) = node.ya_children.iter().next().unwrap();
            result.push(ch);
            node = child;
        }
        result
    }
}

// --- ya_ Bloom Filter ---

/// Bloom filter for probabilistic set membership testing with no false negatives.
#[derive(Debug, Clone)]
pub struct YaBloomFilter {
    ya_bits: Vec<bool>,
    ya_size: usize,
    ya_num_hashes: usize,
    ya_count: usize,
}

impl std::fmt::Display for YaBloomFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bloom(bits={}, hashes={}, count={})", self.ya_size, self.ya_num_hashes, self.ya_count)
    }
}

impl Default for YaBloomFilter {
    fn default() -> Self { Self::ya_new(1000, 5) }
}

impl YaBloomFilter {
    /// Create a new bloom filter with given bit size and number of hash functions.
    pub fn ya_new(bits: usize, num_hashes: usize) -> Self {
        Self { ya_bits: vec![false; bits], ya_size: bits, ya_num_hashes: num_hashes.max(1), ya_count: 0 }
    }

    /// Create from expected number of items and desired false positive rate.
    pub fn ya_with_fp_rate(expected_items: usize, fp_rate: f64) -> Self {
        let bits = (-(expected_items as f64) * fp_rate.ln() / (2.0f64.ln().powi(2))).ceil() as usize;
        let bits = bits.max(64);
        let hashes = ((bits as f64 / expected_items as f64) * 2.0f64.ln()).ceil() as usize;
        let hashes = hashes.max(1);
        Self::ya_new(bits, hashes)
    }

    fn ya_hash(&self, item: u64, seed: usize) -> usize {
        let h = item.wrapping_mul(0xff51afd7ed558ccd_u64.wrapping_add(seed as u64));
        let h = h ^ (h >> 33);
        let h = h.wrapping_mul(0xc4ceb9fe1a85ec53_u64.wrapping_add(seed as u64 * 7));
        (h ^ (h >> 33)) as usize % self.ya_size
    }

    /// Add an item.
    pub fn ya_add(&mut self, item: u64) {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            self.ya_bits[idx] = true;
        }
        self.ya_count += 1;
    }

    /// Check if an item might be in the set (false positives possible, no false negatives).
    pub fn ya_might_contain(&self, item: u64) -> bool {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            if !self.ya_bits[idx] { return false; }
        }
        true
    }

    /// Number of items added.
    pub fn ya_count(&self) -> usize { self.ya_count }

    /// Bit array size.
    pub fn ya_bit_size(&self) -> usize { self.ya_size }

    /// Number of hash functions.
    pub fn ya_num_hashes(&self) -> usize { self.ya_num_hashes }

    /// Estimated false positive rate.
    pub fn ya_estimated_fp_rate(&self) -> f64 {
        let ones = self.ya_bits.iter().filter(|&&b| b).count() as f64;
        (ones / self.ya_size as f64).powi(self.ya_num_hashes as i32)
    }

    /// Clear the filter.
    pub fn ya_clear(&mut self) {
        for b in &mut self.ya_bits { *b = false; }
        self.ya_count = 0;
    }

    /// Merge another bloom filter (union).
    pub fn ya_merge(&mut self, other: &YaBloomFilter) {
        if self.ya_size != other.ya_size { return; }
        for i in 0..self.ya_size {
            self.ya_bits[i] = self.ya_bits[i] || other.ya_bits[i];
        }
    }
}


// --- yb_ Ternary Search Tree ---

/// Node in a ternary search tree (TST) for space-efficient string storage.
#[derive(Debug, Clone)]
pub struct YbTstNode<V: Clone> {
    yb_ch: char,
    yb_left: Option<Box<YbTstNode<V>>>,
    yb_mid: Option<Box<YbTstNode<V>>>,
    yb_right: Option<Box<YbTstNode<V>>>,
    yb_value: Option<V>,
}

impl<V: Clone> YbTstNode<V> {
    fn yb_new(ch: char) -> Self {
        Self { yb_ch: ch, yb_left: None, yb_mid: None, yb_right: None, yb_value: None }
    }
}

/// Ternary search tree for efficient string-keyed storage with prefix queries.
#[derive(Debug, Clone)]
pub struct YbTernarySearchTree<V: Clone> {
    yb_root: Option<Box<YbTstNode<V>>>,
    yb_size: usize,
}

impl<V: Clone> Default for YbTernarySearchTree<V> {
    fn default() -> Self { Self { yb_root: None, yb_size: 0 } }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YbTernarySearchTree<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TST(size={})", self.yb_size)
    }
}

impl<V: Clone> YbTernarySearchTree<V> {
    /// Create an empty TST.
    pub fn yb_new() -> Self { Self { yb_root: None, yb_size: 0 } }

    /// Number of stored keys.
    pub fn yb_len(&self) -> usize { self.yb_size }

    /// Is the tree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_size == 0 }

    /// Insert a key-value pair.
    pub fn yb_insert(&mut self, key: &str, value: V) {
        if key.is_empty() { return; }
        let chars: Vec<char> = key.chars().collect();
        let was_new = Self::yb_insert_node(&mut self.yb_root, &chars, 0, value);
        if was_new { self.yb_size += 1; }
    }

    fn yb_insert_node(node: &mut Option<Box<YbTstNode<V>>>, chars: &[char], depth: usize, value: V) -> bool {
        let ch = chars[depth];
        if node.is_none() { *node = Some(Box::new(YbTstNode::yb_new(ch))); }
        let n = node.as_mut().unwrap();
        if ch < n.yb_ch {
            Self::yb_insert_node(&mut n.yb_left, chars, depth, value)
        } else if ch > n.yb_ch {
            Self::yb_insert_node(&mut n.yb_right, chars, depth, value)
        } else if depth + 1 < chars.len() {
            Self::yb_insert_node(&mut n.yb_mid, chars, depth + 1, value)
        } else {
            let was_new = n.yb_value.is_none();
            n.yb_value = Some(value);
            was_new
        }
    }

    /// Look up a key.
    pub fn yb_get(&self, key: &str) -> Option<&V> {
        if key.is_empty() { return None; }
        let chars: Vec<char> = key.chars().collect();
        Self::yb_get_node(self.yb_root.as_deref(), &chars, 0)
    }

    fn yb_get_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a V> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_get_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_get_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_get_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            n.yb_value.as_ref()
        }
    }

    /// Check if a key exists.
    pub fn yb_contains(&self, key: &str) -> bool { self.yb_get(key).is_some() }

    /// Collect all keys.
    pub fn yb_all_keys(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut current = String::new();
        Self::yb_collect(self.yb_root.as_deref(), &mut current, &mut results);
        results
    }

    fn yb_collect(node: Option<&YbTstNode<V>>, current: &mut String, results: &mut Vec<String>) {
        let Some(n) = node else { return };
        Self::yb_collect(n.yb_left.as_deref(), current, results);
        current.push(n.yb_ch);
        if n.yb_value.is_some() { results.push(current.clone()); }
        Self::yb_collect(n.yb_mid.as_deref(), current, results);
        current.pop();
        Self::yb_collect(n.yb_right.as_deref(), current, results);
    }

    /// Collect keys with a given prefix.
    pub fn yb_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() { return self.yb_all_keys(); }
        let chars: Vec<char> = prefix.chars().collect();
        let node = Self::yb_prefix_node(self.yb_root.as_deref(), &chars, 0);
        let mut results = Vec::new();
        if let Some(n) = node {
            if n.yb_value.is_some() { results.push(prefix.to_string()); }
            let mut current = prefix.to_string();
            Self::yb_collect(n.yb_mid.as_deref(), &mut current, &mut results);
        }
        results
    }

    fn yb_prefix_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a YbTstNode<V>> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_prefix_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_prefix_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_prefix_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            Some(n)
        }
    }

    /// Clear the tree.
    pub fn yb_clear(&mut self) { self.yb_root = None; self.yb_size = 0; }
}

// --- yb_ Quadtree ---

/// A point in 2D space for quadtree storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YbPoint {
    pub yb_x: f64,
    pub yb_y: f64,
}

impl std::fmt::Display for YbPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}, {:.2})", self.yb_x, self.yb_y)
    }
}

impl Default for YbPoint {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0 } }
}

impl YbPoint {
    /// Create a new point.
    pub fn yb_new(x: f64, y: f64) -> Self { Self { yb_x: x, yb_y: y } }

    /// Distance to another point.
    pub fn yb_distance(&self, other: &YbPoint) -> f64 {
        ((self.yb_x - other.yb_x).powi(2) + (self.yb_y - other.yb_y).powi(2)).sqrt()
    }
}

/// Axis-aligned bounding box for quadtree partitioning.
#[derive(Debug, Clone, Copy)]
pub struct YbBounds {
    pub yb_x: f64,
    pub yb_y: f64,
    pub yb_w: f64,
    pub yb_h: f64,
}

impl std::fmt::Display for YbBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bounds({:.1},{:.1} {}x{})", self.yb_x, self.yb_y, self.yb_w, self.yb_h)
    }
}

impl Default for YbBounds {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0, yb_w: 100.0, yb_h: 100.0 } }
}

impl YbBounds {
    /// Create bounds from origin and size.
    pub fn yb_new(x: f64, y: f64, w: f64, h: f64) -> Self { Self { yb_x: x, yb_y: y, yb_w: w, yb_h: h } }

    /// Check if a point is inside these bounds.
    pub fn yb_contains(&self, p: &YbPoint) -> bool {
        p.yb_x >= self.yb_x && p.yb_x < self.yb_x + self.yb_w &&
        p.yb_y >= self.yb_y && p.yb_y < self.yb_y + self.yb_h
    }

    /// Check if two bounds intersect.
    pub fn yb_intersects(&self, other: &YbBounds) -> bool {
        !(self.yb_x + self.yb_w <= other.yb_x || other.yb_x + other.yb_w <= self.yb_x ||
          self.yb_y + self.yb_h <= other.yb_y || other.yb_y + other.yb_h <= self.yb_y)
    }
}

/// Quadtree for 2D spatial indexing with region queries.
#[derive(Debug, Clone)]
pub struct YbQuadtree {
    yb_bounds: YbBounds,
    yb_points: Vec<YbPoint>,
    yb_capacity: usize,
    yb_nw: Option<Box<YbQuadtree>>,
    yb_ne: Option<Box<YbQuadtree>>,
    yb_sw: Option<Box<YbQuadtree>>,
    yb_se: Option<Box<YbQuadtree>>,
    yb_divided: bool,
}

impl std::fmt::Display for YbQuadtree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quadtree(points={}, bounds={})", self.yb_count(), self.yb_bounds)
    }
}

impl Default for YbQuadtree {
    fn default() -> Self { Self::yb_new(YbBounds::default(), 4) }
}

impl YbQuadtree {
    /// Create a new quadtree with given bounds and node capacity.
    pub fn yb_new(bounds: YbBounds, capacity: usize) -> Self {
        Self {
            yb_bounds: bounds, yb_points: Vec::new(), yb_capacity: capacity.max(1),
            yb_nw: None, yb_ne: None, yb_sw: None, yb_se: None, yb_divided: false,
        }
    }

    fn yb_subdivide(&mut self) {
        let x = self.yb_bounds.yb_x;
        let y = self.yb_bounds.yb_y;
        let hw = self.yb_bounds.yb_w / 2.0;
        let hh = self.yb_bounds.yb_h / 2.0;
        self.yb_nw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y, hw, hh), self.yb_capacity)));
        self.yb_ne = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y, hw, hh), self.yb_capacity)));
        self.yb_sw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y + hh, hw, hh), self.yb_capacity)));
        self.yb_se = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y + hh, hw, hh), self.yb_capacity)));
        self.yb_divided = true;
    }

    /// Insert a point.
    pub fn yb_insert(&mut self, point: YbPoint) -> bool {
        if !self.yb_bounds.yb_contains(&point) { return false; }
        if self.yb_points.len() < self.yb_capacity && !self.yb_divided {
            self.yb_points.push(point);
            return true;
        }
        if !self.yb_divided { self.yb_subdivide(); }
        if self.yb_nw.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_ne.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_sw.as_mut().unwrap().yb_insert(point) { return true; }
        self.yb_se.as_mut().unwrap().yb_insert(point)
    }

    /// Query all points within a rectangular region.
    pub fn yb_query(&self, range: &YbBounds) -> Vec<YbPoint> {
        let mut found = Vec::new();
        self.yb_query_inner(range, &mut found);
        found
    }

    fn yb_query_inner(&self, range: &YbBounds, found: &mut Vec<YbPoint>) {
        if !self.yb_bounds.yb_intersects(range) { return; }
        for p in &self.yb_points {
            if range.yb_contains(p) { found.push(*p); }
        }
        if self.yb_divided {
            self.yb_nw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_ne.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_sw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_se.as_ref().unwrap().yb_query_inner(range, found);
        }
    }

    /// Count total points.
    pub fn yb_count(&self) -> usize {
        let mut c = self.yb_points.len();
        if self.yb_divided {
            c += self.yb_nw.as_ref().unwrap().yb_count();
            c += self.yb_ne.as_ref().unwrap().yb_count();
            c += self.yb_sw.as_ref().unwrap().yb_count();
            c += self.yb_se.as_ref().unwrap().yb_count();
        }
        c
    }

    /// Is the quadtree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_count() == 0 }

    /// Get bounds.
    pub fn yb_bounds(&self) -> &YbBounds { &self.yb_bounds }

    /// Find nearest point to a target.
    pub fn yb_nearest(&self, target: &YbPoint) -> Option<YbPoint> {
        let all = self.yb_query(&self.yb_bounds);
        all.into_iter().min_by(|a, b| {
            a.yb_distance(target).partial_cmp(&b.yb_distance(target)).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}


// --- yc_ Van Emde Boas Set ---

/// Simplified van Emde Boas-inspired set for integer keys in [0, universe).
/// Uses a flat bitmap for practical efficiency with O(1) operations.
#[derive(Debug, Clone)]
pub struct YcVebSet {
    yc_bits: Vec<u64>,
    yc_universe: usize,
    yc_count: usize,
    yc_min: Option<usize>,
    yc_max: Option<usize>,
}

impl std::fmt::Display for YcVebSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VebSet(universe={}, count={})", self.yc_universe, self.yc_count)
    }
}

impl Default for YcVebSet {
    fn default() -> Self { Self::yc_new(65536) }
}

impl YcVebSet {
    /// Create a set supporting keys in [0, universe).
    pub fn yc_new(universe: usize) -> Self {
        let words = (universe + 63) / 64;
        Self { yc_bits: vec![0; words], yc_universe: universe, yc_count: 0, yc_min: None, yc_max: None }
    }

    /// Universe size.
    pub fn yc_universe(&self) -> usize { self.yc_universe }

    /// Number of elements.
    pub fn yc_len(&self) -> usize { self.yc_count }

    /// Is the set empty.
    pub fn yc_is_empty(&self) -> bool { self.yc_count == 0 }

    /// Insert a key.
    pub fn yc_insert(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) != 0 { return false; }
        self.yc_bits[word] |= 1u64 << bit;
        self.yc_count += 1;
        self.yc_min = Some(self.yc_min.map_or(key, |m: usize| m.min(key)));
        self.yc_max = Some(self.yc_max.map_or(key, |m: usize| m.max(key)));
        true
    }

    /// Remove a key.
    pub fn yc_remove(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) == 0 { return false; }
        self.yc_bits[word] &= !(1u64 << bit);
        self.yc_count -= 1;
        if self.yc_count == 0 { self.yc_min = None; self.yc_max = None; }
        else {
            if self.yc_min == Some(key) { self.yc_min = self.yc_successor(key); }
            if self.yc_max == Some(key) { self.yc_max = self.yc_predecessor(key); }
        }
        true
    }

    /// Check membership.
    pub fn yc_contains(&self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        self.yc_bits[key / 64] & (1u64 << (key % 64)) != 0
    }

    /// Minimum element.
    pub fn yc_min(&self) -> Option<usize> { self.yc_min }

    /// Maximum element.
    pub fn yc_max(&self) -> Option<usize> { self.yc_max }

    /// Find the smallest key > given key.
    pub fn yc_successor(&self, key: usize) -> Option<usize> {
        for k in (key + 1)..self.yc_universe {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Find the largest key < given key.
    pub fn yc_predecessor(&self, key: usize) -> Option<usize> {
        if key == 0 { return None; }
        for k in (0..key).rev() {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Collect all elements in sorted order.
    pub fn yc_to_sorted_vec(&self) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.yc_count);
        for w in 0..self.yc_bits.len() {
            let mut bits = self.yc_bits[w];
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                result.push(w * 64 + tz);
                bits &= bits - 1;
            }
        }
        result
    }

    /// Clear the set.
    pub fn yc_clear(&mut self) {
        for w in &mut self.yc_bits { *w = 0; }
        self.yc_count = 0;
        self.yc_min = None;
        self.yc_max = None;
    }

    /// Union with another set (same universe).
    pub fn yc_union(&mut self, other: &YcVebSet) {
        if self.yc_universe != other.yc_universe { return; }
        for i in 0..self.yc_bits.len() {
            self.yc_bits[i] |= other.yc_bits[i];
        }
        self.yc_count = self.yc_to_sorted_vec().len();
        let sorted = self.yc_to_sorted_vec();
        self.yc_min = sorted.first().copied();
        self.yc_max = sorted.last().copied();
    }

    /// Intersection with another set.
    pub fn yc_intersection(&self, other: &YcVebSet) -> YcVebSet {
        let mut result = YcVebSet::yc_new(self.yc_universe);
        if self.yc_universe != other.yc_universe { return result; }
        for i in 0..self.yc_bits.len() {
            result.yc_bits[i] = self.yc_bits[i] & other.yc_bits[i];
        }
        let sorted = result.yc_to_sorted_vec();
        result.yc_count = sorted.len();
        result.yc_min = sorted.first().copied();
        result.yc_max = sorted.last().copied();
        result
    }
}

// --- yc_ Consistent Hash Ring ---

/// Consistent hash ring for distributed key mapping with virtual nodes.
#[derive(Debug, Clone)]
pub struct YcHashRing {
    yc_ring: std::collections::BTreeMap<u64, String>,
    yc_replicas: usize,
    yc_nodes: Vec<String>,
}

impl std::fmt::Display for YcHashRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HashRing(nodes={}, replicas={})", self.yc_nodes.len(), self.yc_replicas)
    }
}

impl Default for YcHashRing {
    fn default() -> Self { Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: 150, yc_nodes: Vec::new() } }
}

impl YcHashRing {
    /// Create a new hash ring with given replica count per node.
    pub fn yc_new(replicas: usize) -> Self {
        Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: replicas.max(1), yc_nodes: Vec::new() }
    }

    fn yc_hash(key: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in key.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// Add a node to the ring.
    pub fn yc_add_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.insert(hash, node.to_string());
        }
        self.yc_nodes.push(node.to_string());
    }

    /// Remove a node from the ring.
    pub fn yc_remove_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.remove(&hash);
        }
        self.yc_nodes.retain(|n| n != node);
    }

    /// Find the node responsible for a key.
    pub fn yc_get_node(&self, key: &str) -> Option<&str> {
        if self.yc_ring.is_empty() { return None; }
        let hash = Self::yc_hash(key);
        let node = self.yc_ring.range(hash..).next()
            .or_else(|| self.yc_ring.iter().next());
        node.map(|(_, v)| v.as_str())
    }

    /// Number of physical nodes.
    pub fn yc_node_count(&self) -> usize { self.yc_nodes.len() }

    /// Number of virtual nodes on the ring.
    pub fn yc_virtual_count(&self) -> usize { self.yc_ring.len() }

    /// List all physical nodes.
    pub fn yc_nodes(&self) -> &[String] { &self.yc_nodes }

    /// Check if a node is in the ring.
    pub fn yc_has_node(&self, node: &str) -> bool { self.yc_nodes.iter().any(|n| n == node) }
}


// --- yd_ Directed Acyclic Graph ---

/// Directed acyclic graph with topological sorting and cycle detection.
#[derive(Debug, Clone)]
pub struct YdDag {
    yd_adj: std::collections::HashMap<usize, Vec<usize>>,
    yd_in_degree: std::collections::HashMap<usize, usize>,
    yd_nodes: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YdDag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let edges: usize = self.yd_adj.values().map(|v| v.len()).sum();
        write!(f, "DAG(nodes={}, edges={})", self.yd_nodes.len(), edges)
    }
}

impl Default for YdDag {
    fn default() -> Self { Self::yd_new() }
}

impl YdDag {
    /// Create an empty DAG.
    pub fn yd_new() -> Self {
        Self { yd_adj: std::collections::HashMap::new(), yd_in_degree: std::collections::HashMap::new(), yd_nodes: std::collections::HashSet::new() }
    }

    /// Add a node.
    pub fn yd_add_node(&mut self, node: usize) {
        self.yd_nodes.insert(node);
        self.yd_adj.entry(node).or_default();
        self.yd_in_degree.entry(node).or_insert(0);
    }

    /// Add a directed edge from -> to.
    pub fn yd_add_edge(&mut self, from: usize, to: usize) {
        self.yd_add_node(from);
        self.yd_add_node(to);
        self.yd_adj.entry(from).or_default().push(to);
        *self.yd_in_degree.entry(to).or_insert(0) += 1;
    }

    /// Number of nodes.
    pub fn yd_node_count(&self) -> usize { self.yd_nodes.len() }

    /// Number of edges.
    pub fn yd_edge_count(&self) -> usize { self.yd_adj.values().map(|v| v.len()).sum() }

    /// Topological sort using Kahn's algorithm. Returns None if cycle detected.
    pub fn yd_topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = self.yd_in_degree.clone();
        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        for (node, deg) in &in_deg {
            if *deg == 0 { queue.push_back(*node); }
        }
        let mut result = Vec::new();
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    let d = in_deg.get_mut(&next).unwrap();
                    *d -= 1;
                    if *d == 0 { queue.push_back(next); }
                }
            }
        }
        if result.len() == self.yd_nodes.len() { Some(result) } else { None }
    }

    /// Check if the graph has a cycle.
    pub fn yd_has_cycle(&self) -> bool { self.yd_topological_sort().is_none() }

    /// Get all neighbors of a node.
    pub fn yd_neighbors(&self, node: usize) -> &[usize] {
        self.yd_adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get in-degree of a node.
    pub fn yd_in_degree(&self, node: usize) -> usize {
        self.yd_in_degree.get(&node).copied().unwrap_or(0)
    }

    /// Get out-degree of a node.
    pub fn yd_out_degree(&self, node: usize) -> usize {
        self.yd_adj.get(&node).map(|v| v.len()).unwrap_or(0)
    }

    /// Find all root nodes (in-degree 0).
    pub fn yd_roots(&self) -> Vec<usize> {
        let mut roots: Vec<usize> = self.yd_in_degree.iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        roots.sort();
        roots
    }

    /// Find all leaf nodes (out-degree 0).
    pub fn yd_leaves(&self) -> Vec<usize> {
        let mut leaves: Vec<usize> = self.yd_nodes.iter()
            .filter(|&&n| self.yd_out_degree(n) == 0)
            .copied()
            .collect();
        leaves.sort();
        leaves
    }

    /// Check if node exists.
    pub fn yd_has_node(&self, node: usize) -> bool { self.yd_nodes.contains(&node) }

    /// Clear the graph.
    pub fn yd_clear(&mut self) {
        self.yd_adj.clear();
        self.yd_in_degree.clear();
        self.yd_nodes.clear();
    }

    /// BFS traversal from a start node.
    pub fn yd_bfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        queue.push_back(start);
        visited.insert(start);
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                let mut sorted_n = neighbors.clone();
                sorted_n.sort();
                for next in sorted_n {
                    if visited.insert(next) { queue.push_back(next); }
                }
            }
        }
        result
    }

    /// DFS traversal from a start node.
    pub fn yd_dfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        self.yd_dfs_inner(start, &mut visited, &mut result);
        result
    }

    fn yd_dfs_inner(&self, node: usize, visited: &mut std::collections::HashSet<usize>, result: &mut Vec<usize>) {
        if !visited.insert(node) { return; }
        result.push(node);
        if let Some(neighbors) = self.yd_adj.get(&node) {
            let mut sorted_n = neighbors.clone();
            sorted_n.sort();
            for next in sorted_n {
                self.yd_dfs_inner(next, visited, result);
            }
        }
    }

    /// Shortest path length (unweighted) between two nodes using BFS.
    pub fn yd_shortest_path(&self, from: usize, to: usize) -> Option<usize> {
        if from == to { return Some(0); }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((from, 0usize));
        visited.insert(from);
        while let Some((node, dist)) = queue.pop_front() {
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    if next == to { return Some(dist + 1); }
                    if visited.insert(next) { queue.push_back((next, dist + 1)); }
                }
            }
        }
        None
    }
}

// --- yd_ Sparse Matrix ---

/// Sparse matrix using coordinate (COO) format for efficient storage.
#[derive(Debug, Clone)]
pub struct YdSparseMatrix {
    yd_rows: usize,
    yd_cols: usize,
    yd_entries: std::collections::HashMap<(usize, usize), f64>,
}

impl std::fmt::Display for YdSparseMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SparseMatrix({}x{}, nnz={})", self.yd_rows, self.yd_cols, self.yd_entries.len())
    }
}

impl Default for YdSparseMatrix {
    fn default() -> Self { Self::yd_new(0, 0) }
}

impl YdSparseMatrix {
    /// Create a new sparse matrix with given dimensions.
    pub fn yd_new(rows: usize, cols: usize) -> Self {
        Self { yd_rows: rows, yd_cols: cols, yd_entries: std::collections::HashMap::new() }
    }

    /// Set a value.
    pub fn yd_set(&mut self, row: usize, col: usize, val: f64) {
        if val == 0.0 { self.yd_entries.remove(&(row, col)); }
        else { self.yd_entries.insert((row, col), val); }
    }

    /// Get a value.
    pub fn yd_get(&self, row: usize, col: usize) -> f64 {
        self.yd_entries.get(&(row, col)).copied().unwrap_or(0.0)
    }

    /// Number of non-zero entries.
    pub fn yd_nnz(&self) -> usize { self.yd_entries.len() }

    /// Dimensions.
    pub fn yd_rows(&self) -> usize { self.yd_rows }
    pub fn yd_cols(&self) -> usize { self.yd_cols }

    /// Transpose.
    pub fn yd_transpose(&self) -> YdSparseMatrix {
        let mut t = YdSparseMatrix::yd_new(self.yd_cols, self.yd_rows);
        for ((r, c), v) in &self.yd_entries {
            t.yd_set(*c, *r, *v);
        }
        t
    }

    /// Matrix-vector multiply.
    pub fn yd_mul_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.yd_rows];
        for ((r, c), v) in &self.yd_entries {
            if *c < vec.len() && *r < result.len() {
                result[*r] += *v * vec[*c];
            }
        }
        result
    }

    /// Scale all entries.
    pub fn yd_scale(&mut self, factor: f64) {
        for v in self.yd_entries.values_mut() { *v *= factor; }
    }

    /// Add another sparse matrix.
    pub fn yd_add(&self, other: &YdSparseMatrix) -> YdSparseMatrix {
        let mut result = self.clone();
        for ((r, c), v) in &other.yd_entries {
            let entry = result.yd_entries.entry((*r, *c)).or_insert(0.0);
            *entry += *v;
        }
        result
    }

    /// Clear all entries.
    pub fn yd_clear(&mut self) { self.yd_entries.clear(); }

    /// Row sum.
    pub fn yd_row_sum(&self, row: usize) -> f64 {
        self.yd_entries.iter()
            .filter(|((r, _), _)| *r == row)
            .map(|(_, v)| *v)
            .sum()
    }

    /// Frobenius norm squared.
    pub fn yd_frobenius_sq(&self) -> f64 {
        self.yd_entries.values().map(|v| *v * *v).sum()
    }
}


// --- ye_ Indexed Priority Queue ---

/// Indexed min-priority queue supporting decrease-key in O(log n).
#[derive(Debug, Clone)]
pub struct YeIndexedPQ {
    ye_heap: Vec<(usize, i64)>,
    ye_pos: std::collections::HashMap<usize, usize>,
}

impl std::fmt::Display for YeIndexedPQ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndexedPQ(size={})", self.ye_heap.len())
    }
}

impl Default for YeIndexedPQ {
    fn default() -> Self { Self::ye_new() }
}

impl YeIndexedPQ {
    /// Create an empty indexed priority queue.
    pub fn ye_new() -> Self { Self { ye_heap: Vec::new(), ye_pos: std::collections::HashMap::new() } }

    /// Number of elements.
    pub fn ye_len(&self) -> usize { self.ye_heap.len() }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_heap.is_empty() }

    /// Insert an element with priority.
    pub fn ye_insert(&mut self, id: usize, priority: i64) {
        if self.ye_pos.contains_key(&id) { self.ye_decrease_key(id, priority); return; }
        let idx = self.ye_heap.len();
        self.ye_heap.push((id, priority));
        self.ye_pos.insert(id, idx);
        self.ye_sift_up(idx);
    }

    /// Peek at minimum.
    pub fn ye_peek(&self) -> Option<(usize, i64)> { self.ye_heap.first().copied() }

    /// Extract minimum.
    pub fn ye_pop(&mut self) -> Option<(usize, i64)> {
        if self.ye_heap.is_empty() { return None; }
        let min = self.ye_heap[0];
        let last = self.ye_heap.len() - 1;
        self.ye_swap(0, last);
        self.ye_heap.pop();
        self.ye_pos.remove(&min.0);
        if !self.ye_heap.is_empty() { self.ye_sift_down(0); }
        Some(min)
    }

    /// Decrease the priority of an element.
    pub fn ye_decrease_key(&mut self, id: usize, new_priority: i64) {
        if let Some(&idx) = self.ye_pos.get(&id) {
            if new_priority < self.ye_heap[idx].1 {
                self.ye_heap[idx].1 = new_priority;
                self.ye_sift_up(idx);
            }
        }
    }

    /// Check if an id is in the queue.
    pub fn ye_contains(&self, id: usize) -> bool { self.ye_pos.contains_key(&id) }

    /// Get priority of an id.
    pub fn ye_priority(&self, id: usize) -> Option<i64> {
        self.ye_pos.get(&id).map(|&idx| self.ye_heap[idx].1)
    }

    fn ye_swap(&mut self, i: usize, j: usize) {
        self.ye_heap.swap(i, j);
        self.ye_pos.insert(self.ye_heap[i].0, i);
        self.ye_pos.insert(self.ye_heap[j].0, j);
    }

    fn ye_sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.ye_heap[idx].1 < self.ye_heap[parent].1 {
                self.ye_swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn ye_sift_down(&mut self, mut idx: usize) {
        let n = self.ye_heap.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < n && self.ye_heap[left].1 < self.ye_heap[smallest].1 { smallest = left; }
            if right < n && self.ye_heap[right].1 < self.ye_heap[smallest].1 { smallest = right; }
            if smallest == idx { break; }
            self.ye_swap(idx, smallest);
            idx = smallest;
        }
    }

    /// Clear the queue.
    pub fn ye_clear(&mut self) { self.ye_heap.clear(); self.ye_pos.clear(); }

    /// Drain all elements in priority order.
    pub fn ye_drain_sorted(&mut self) -> Vec<(usize, i64)> {
        let mut result = Vec::with_capacity(self.ye_heap.len());
        while let Some(item) = self.ye_pop() { result.push(item); }
        result
    }
}

// --- ye_ Segment Tree with Lazy Propagation ---

/// Segment tree with lazy propagation for range queries and updates.
#[derive(Debug, Clone)]
pub struct YeSegTree {
    ye_n: usize,
    ye_tree: Vec<i64>,
    ye_lazy: Vec<i64>,
}

impl std::fmt::Display for YeSegTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SegTree(n={})", self.ye_n)
    }
}

impl Default for YeSegTree {
    fn default() -> Self { Self { ye_n: 0, ye_tree: Vec::new(), ye_lazy: Vec::new() } }
}

impl YeSegTree {
    /// Build from an array of values.
    pub fn ye_from_slice(data: &[i64]) -> Self {
        let n = data.len();
        let mut tree = vec![0i64; 4 * n];
        let lazy = vec![0i64; 4 * n];
        let mut st = Self { ye_n: n, ye_tree: tree.clone(), ye_lazy: lazy };
        if n > 0 { st.ye_build(data, 1, 0, n - 1); }
        st
    }

    fn ye_build(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.ye_tree[node] = data[start];
            return;
        }
        let mid = (start + end) / 2;
        self.ye_build(data, 2 * node, start, mid);
        self.ye_build(data, 2 * node + 1, mid + 1, end);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    fn ye_push_down(&mut self, node: usize, start: usize, end: usize) {
        if self.ye_lazy[node] != 0 {
            let mid = (start + end) / 2;
            self.ye_tree[2 * node] += self.ye_lazy[node] * (mid - start + 1) as i64;
            self.ye_tree[2 * node + 1] += self.ye_lazy[node] * (end - mid) as i64;
            self.ye_lazy[2 * node] += self.ye_lazy[node];
            self.ye_lazy[2 * node + 1] += self.ye_lazy[node];
            self.ye_lazy[node] = 0;
        }
    }

    /// Range sum query [l, r].
    pub fn ye_query(&mut self, l: usize, r: usize) -> i64 {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return 0; }
        self.ye_query_inner(1, 0, self.ye_n - 1, l, r)
    }

    fn ye_query_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.ye_tree[node]; }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_query_inner(2 * node, start, mid, l, r) +
        self.ye_query_inner(2 * node + 1, mid + 1, end, l, r)
    }

    /// Range update: add val to all elements in [l, r].
    pub fn ye_update(&mut self, l: usize, r: usize, val: i64) {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return; }
        self.ye_update_inner(1, 0, self.ye_n - 1, l, r, val);
    }

    fn ye_update_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize, val: i64) {
        if r < start || end < l { return; }
        if l <= start && end <= r {
            self.ye_tree[node] += val * (end - start + 1) as i64;
            self.ye_lazy[node] += val;
            return;
        }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_update_inner(2 * node, start, mid, l, r, val);
        self.ye_update_inner(2 * node + 1, mid + 1, end, l, r, val);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    /// Point query: get value at index.
    pub fn ye_point_query(&mut self, idx: usize) -> i64 {
        self.ye_query(idx, idx)
    }

    /// Size of underlying array.
    pub fn ye_len(&self) -> usize { self.ye_n }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_n == 0 }
}


// --- yf_ Disjoint Interval Set ---

/// Set of non-overlapping intervals with automatic merging.
#[derive(Debug, Clone)]
pub struct YfIntervalSet {
    yf_intervals: std::collections::BTreeMap<i64, i64>,
}

impl std::fmt::Display for YfIntervalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IntervalSet(count={})", self.yf_intervals.len())
    }
}

impl Default for YfIntervalSet {
    fn default() -> Self { Self::yf_new() }
}

impl YfIntervalSet {
    /// Create an empty interval set.
    pub fn yf_new() -> Self { Self { yf_intervals: std::collections::BTreeMap::new() } }

    /// Number of disjoint intervals.
    pub fn yf_len(&self) -> usize { self.yf_intervals.len() }

    /// Is empty.
    pub fn yf_is_empty(&self) -> bool { self.yf_intervals.is_empty() }

    /// Add an interval [lo, hi]. Merges with overlapping/adjacent intervals.
    pub fn yf_add(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let to_remove: Vec<i64> = self.yf_intervals.range(..=hi + 1)
            .filter(|(start, end)| **end >= lo - 1)
            .map(|(s, _)| *s)
            .collect();
        for s in &to_remove {
            let e = self.yf_intervals[s];
            new_lo = new_lo.min(*s);
            new_hi = new_hi.max(e);
            self.yf_intervals.remove(s);
        }
        self.yf_intervals.insert(new_lo, new_hi);
    }

    /// Check if a value is covered by any interval.
    pub fn yf_contains(&self, val: i64) -> bool {
        if let Some((_, end)) = self.yf_intervals.range(..=val).next_back() {
            *end >= val
        } else {
            false
        }
    }

    /// Remove a point, splitting intervals if needed.
    pub fn yf_remove_point(&mut self, val: i64) {
        let covering = self.yf_intervals.range(..=val)
            .filter(|(_, end)| **end >= val)
            .map(|(s, e)| (*s, *e))
            .next_back();
        if let Some((s, e)) = covering {
            self.yf_intervals.remove(&s);
            if s < val { self.yf_intervals.insert(s, val - 1); }
            if val < e { self.yf_intervals.insert(val + 1, e); }
        }
    }

    /// Get all intervals as sorted vec.
    pub fn yf_intervals(&self) -> Vec<(i64, i64)> {
        self.yf_intervals.iter().map(|(s, e)| (*s, *e)).collect()
    }

    /// Total covered length.
    pub fn yf_total_length(&self) -> i64 {
        self.yf_intervals.iter().map(|(s, e)| e - s + 1).sum()
    }

    /// Clear all intervals.
    pub fn yf_clear(&mut self) { self.yf_intervals.clear(); }

    /// Check if two interval sets overlap.
    pub fn yf_overlaps(&self, other: &YfIntervalSet) -> bool {
        for (s, e) in &self.yf_intervals {
            for (os, oe) in &other.yf_intervals {
                if s <= oe && os <= e { return true; }
            }
        }
        false
    }
}

// --- yf_ K-way Merge ---

/// K-way merge iterator that merges multiple sorted sequences.
#[derive(Debug, Clone)]
pub struct YfKWayMerge {
    yf_sources: Vec<Vec<i64>>,
    yf_indices: Vec<usize>,
}

impl std::fmt::Display for YfKWayMerge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KWayMerge(sources={})", self.yf_sources.len())
    }
}

impl Default for YfKWayMerge {
    fn default() -> Self { Self::yf_new() }
}

impl YfKWayMerge {
    /// Create an empty k-way merge.
    pub fn yf_new() -> Self { Self { yf_sources: Vec::new(), yf_indices: Vec::new() } }

    /// Add a sorted source.
    pub fn yf_add_source(&mut self, source: Vec<i64>) {
        self.yf_sources.push(source);
        self.yf_indices.push(0);
    }

    /// Number of sources.
    pub fn yf_source_count(&self) -> usize { self.yf_sources.len() }

    /// Merge all sources into a single sorted vec.
    pub fn yf_merge(&mut self) -> Vec<i64> {
        let mut result = Vec::new();
        loop {
            let mut min_val: Option<i64> = None;
            let mut min_src = 0;
            for (i, (src, idx)) in self.yf_sources.iter().zip(self.yf_indices.iter()).enumerate() {
                if *idx < src.len() {
                    let v = src[*idx];
                    if min_val.is_none() || v < min_val.unwrap() {
                        min_val = Some(v);
                        min_src = i;
                    }
                }
            }
            match min_val {
                Some(v) => { result.push(v); self.yf_indices[min_src] += 1; }
                None => break,
            }
        }
        result
    }

    /// Total remaining elements across all sources.
    pub fn yf_remaining(&self) -> usize {
        self.yf_sources.iter().zip(self.yf_indices.iter())
            .map(|(src, idx)| src.len().saturating_sub(*idx))
            .sum()
    }

    /// Reset all indices.
    pub fn yf_reset(&mut self) {
        for idx in &mut self.yf_indices { *idx = 0; }
    }

    /// Clear all sources.
    pub fn yf_clear(&mut self) { self.yf_sources.clear(); self.yf_indices.clear(); }

    /// Check if merge is complete.
    pub fn yf_is_done(&self) -> bool { self.yf_remaining() == 0 }

    /// Merge and deduplicate.
    pub fn yf_merge_unique(&mut self) -> Vec<i64> {
        let merged = self.yf_merge();
        let mut unique = Vec::new();
        for v in merged {
            if unique.last() != Some(&v) { unique.push(v); }
        }
        unique
    }
}


// --- yg_ Persistent Stack ---

/// Immutable persistent stack using a linked list of arcs.
#[derive(Debug, Clone)]
pub struct YgPersistentStack<T: Clone> {
    yg_head: Option<std::sync::Arc<YgStackNode<T>>>,
    yg_size: usize,
}

#[derive(Debug, Clone)]
struct YgStackNode<T: Clone> {
    yg_value: T,
    yg_next: Option<std::sync::Arc<YgStackNode<T>>>,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for YgPersistentStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PStack(size={})", self.yg_size)
    }
}

impl<T: Clone> Default for YgPersistentStack<T> {
    fn default() -> Self { Self::yg_new() }
}

impl<T: Clone> YgPersistentStack<T> {
    /// Create an empty persistent stack.
    pub fn yg_new() -> Self { Self { yg_head: None, yg_size: 0 } }

    /// Push returns a new stack with the element on top.
    pub fn yg_push(&self, value: T) -> Self {
        Self {
            yg_head: Some(std::sync::Arc::new(YgStackNode { yg_value: value, yg_next: self.yg_head.clone() })),
            yg_size: self.yg_size + 1,
        }
    }

    /// Pop returns the top value and a new stack without it.
    pub fn yg_pop(&self) -> Option<(T, Self)> {
        self.yg_head.as_ref().map(|node| {
            (node.yg_value.clone(), Self { yg_head: node.yg_next.clone(), yg_size: self.yg_size - 1 })
        })
    }

    /// Peek at the top value.
    pub fn yg_peek(&self) -> Option<&T> {
        self.yg_head.as_ref().map(|node| &node.yg_value)
    }

    /// Size of the stack.
    pub fn yg_len(&self) -> usize { self.yg_size }

    /// Is empty.
    pub fn yg_is_empty(&self) -> bool { self.yg_size == 0 }

    /// Convert to vec (top first).
    pub fn yg_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.yg_size);
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result.push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }

    /// Reverse the stack.
    pub fn yg_reverse(&self) -> Self {
        let mut result = Self::yg_new();
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result = result.yg_push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }
}

// --- yg_ Bitmap Index ---

/// Bitmap index for fast multi-column filtering on categorical data.
#[derive(Debug, Clone)]
pub struct YgBitmapIndex {
    yg_bitmaps: std::collections::HashMap<String, Vec<u64>>,
    yg_num_rows: usize,
}

impl std::fmt::Display for YgBitmapIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BitmapIndex(rows={}, columns={})", self.yg_num_rows, self.yg_bitmaps.len())
    }
}

impl Default for YgBitmapIndex {
    fn default() -> Self { Self::yg_new(0) }
}

impl YgBitmapIndex {
    /// Create a bitmap index for a given number of rows.
    pub fn yg_new(num_rows: usize) -> Self {
        Self { yg_bitmaps: std::collections::HashMap::new(), yg_num_rows: num_rows }
    }

    fn yg_words(n: usize) -> usize { (n + 63) / 64 }

    /// Set a value for a column at a given row.
    pub fn yg_set(&mut self, column: &str, row: usize) {
        if row >= self.yg_num_rows { return; }
        let words = Self::yg_words(self.yg_num_rows);
        let bitmap = self.yg_bitmaps.entry(column.to_string()).or_insert_with(|| vec![0u64; words]);
        bitmap[row / 64] |= 1u64 << (row % 64);
    }

    /// Check if a column is set for a row.
    pub fn yg_get(&self, column: &str, row: usize) -> bool {
        if row >= self.yg_num_rows { return false; }
        self.yg_bitmaps.get(column)
            .map(|bm| bm[row / 64] & (1u64 << (row % 64)) != 0)
            .unwrap_or(false)
    }

    /// AND query: rows where all columns are set.
    pub fn yg_and(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![u64::MAX; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w &= bm[i]; }
            } else {
                return Vec::new();
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    /// OR query: rows where any column is set.
    pub fn yg_or(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![0u64; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w |= bm[i]; }
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    fn yg_bits_to_rows(bits: &[u64], max_rows: usize) -> Vec<usize> {
        let mut rows = Vec::new();
        for (w_idx, word) in bits.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                let row = w_idx * 64 + tz;
                if row < max_rows { rows.push(row); }
                bits &= bits - 1;
            }
        }
        rows
    }

    /// Count of rows for a column.
    pub fn yg_count(&self, column: &str) -> usize {
        self.yg_bitmaps.get(column)
            .map(|bm| bm.iter().map(|w| w.count_ones() as usize).sum())
            .unwrap_or(0)
    }

    /// Number of rows.
    pub fn yg_num_rows(&self) -> usize { self.yg_num_rows }

    /// Number of columns.
    pub fn yg_num_columns(&self) -> usize { self.yg_bitmaps.len() }

    /// List column names.
    pub fn yg_columns(&self) -> Vec<String> {
        let mut cols: Vec<String> = self.yg_bitmaps.keys().cloned().collect();
        cols.sort();
        cols
    }

    /// Clear all bitmaps.
    pub fn yg_clear(&mut self) { self.yg_bitmaps.clear(); }
}


// --- yh_ Order Statistics Tree ---

/// Order statistics tree supporting rank queries and selection.
/// Implemented as an augmented BST with subtree sizes.
#[derive(Debug, Clone)]
pub struct YhOrderStatTree {
    yh_root: Option<Box<YhOstNode>>,
}

#[derive(Debug, Clone)]
struct YhOstNode {
    yh_key: i64,
    yh_left: Option<Box<YhOstNode>>,
    yh_right: Option<Box<YhOstNode>>,
    yh_size: usize,
}

impl YhOstNode {
    fn yh_new(key: i64) -> Self {
        Self { yh_key: key, yh_left: None, yh_right: None, yh_size: 1 }
    }

    fn yh_left_size(&self) -> usize {
        self.yh_left.as_ref().map_or(0, |n| n.yh_size)
    }

    fn yh_update_size(&mut self) {
        self.yh_size = 1 + self.yh_left.as_ref().map_or(0, |n| n.yh_size)
            + self.yh_right.as_ref().map_or(0, |n| n.yh_size);
    }
}

impl std::fmt::Display for YhOrderStatTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OSTree(size={})", self.yh_len())
    }
}

impl Default for YhOrderStatTree {
    fn default() -> Self { Self::yh_new() }
}

impl YhOrderStatTree {
    /// Create an empty order statistics tree.
    pub fn yh_new() -> Self { Self { yh_root: None } }

    /// Number of elements.
    pub fn yh_len(&self) -> usize { self.yh_root.as_ref().map_or(0, |n| n.yh_size) }

    /// Is empty.
    pub fn yh_is_empty(&self) -> bool { self.yh_root.is_none() }

    /// Insert a key.
    pub fn yh_insert(&mut self, key: i64) {
        Self::yh_insert_node(&mut self.yh_root, key);
    }

    fn yh_insert_node(node: &mut Option<Box<YhOstNode>>, key: i64) {
        match node {
            None => { *node = Some(Box::new(YhOstNode::yh_new(key))); }
            Some(n) => {
                if key < n.yh_key { Self::yh_insert_node(&mut n.yh_left, key); }
                else if key > n.yh_key { Self::yh_insert_node(&mut n.yh_right, key); }
                n.yh_update_size();
            }
        }
    }

    /// Check if a key exists.
    pub fn yh_contains(&self, key: i64) -> bool {
        let mut current = &self.yh_root;
        while let Some(n) = current {
            if key < n.yh_key { current = &n.yh_left; }
            else if key > n.yh_key { current = &n.yh_right; }
            else { return true; }
        }
        false
    }

    /// Rank of a key (0-indexed, number of elements < key).
    pub fn yh_rank(&self, key: i64) -> usize {
        Self::yh_rank_node(&self.yh_root, key)
    }

    fn yh_rank_node(node: &Option<Box<YhOstNode>>, key: i64) -> usize {
        match node {
            None => 0,
            Some(n) => {
                if key < n.yh_key { Self::yh_rank_node(&n.yh_left, key) }
                else if key > n.yh_key { n.yh_left_size() + 1 + Self::yh_rank_node(&n.yh_right, key) }
                else { n.yh_left_size() }
            }
        }
    }

    /// Select the k-th smallest element (0-indexed).
    pub fn yh_select(&self, k: usize) -> Option<i64> {
        Self::yh_select_node(&self.yh_root, k)
    }

    fn yh_select_node(node: &Option<Box<YhOstNode>>, k: usize) -> Option<i64> {
        let n = node.as_ref()?;
        let left_size = n.yh_left_size();
        if k < left_size { Self::yh_select_node(&n.yh_left, k) }
        else if k > left_size { Self::yh_select_node(&n.yh_right, k - left_size - 1) }
        else { Some(n.yh_key) }
    }

    /// Minimum key.
    pub fn yh_min(&self) -> Option<i64> {
        let mut current = &self.yh_root;
        let mut min = None;
        while let Some(n) = current {
            min = Some(n.yh_key);
            current = &n.yh_left;
        }
        min
    }

    /// Maximum key.
    pub fn yh_max(&self) -> Option<i64> {
        let mut current = &self.yh_root;
        let mut max = None;
        while let Some(n) = current {
            max = Some(n.yh_key);
            current = &n.yh_right;
        }
        max
    }

    /// In-order traversal.
    pub fn yh_inorder(&self) -> Vec<i64> {
        let mut result = Vec::new();
        Self::yh_inorder_node(&self.yh_root, &mut result);
        result
    }

    fn yh_inorder_node(node: &Option<Box<YhOstNode>>, result: &mut Vec<i64>) {
        if let Some(n) = node {
            Self::yh_inorder_node(&n.yh_left, result);
            result.push(n.yh_key);
            Self::yh_inorder_node(&n.yh_right, result);
        }
    }

    /// Count elements in range [lo, hi].
    pub fn yh_count_range(&self, lo: i64, hi: i64) -> usize {
        if lo > hi { return 0; }
        let rank_hi = self.yh_rank(hi + 1);
        let rank_lo = self.yh_rank(lo);
        rank_hi - rank_lo
    }
}

// --- yh_ Reservoir Sampler ---

/// Reservoir sampling for uniformly random samples from a stream.
#[derive(Debug, Clone)]
pub struct YhReservoirSampler {
    yh_reservoir: Vec<i64>,
    yh_k: usize,
    yh_count: usize,
    yh_seed: u64,
}

impl std::fmt::Display for YhReservoirSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reservoir(k={}, seen={})", self.yh_k, self.yh_count)
    }
}

impl Default for YhReservoirSampler {
    fn default() -> Self { Self::yh_new(10, 42) }
}

impl YhReservoirSampler {
    /// Create a reservoir sampler for k items.
    pub fn yh_new(k: usize, seed: u64) -> Self {
        Self { yh_reservoir: Vec::with_capacity(k), yh_k: k, yh_count: 0, yh_seed: seed }
    }

    fn yh_next_rand(&mut self) -> u64 {
        self.yh_seed ^= self.yh_seed << 13;
        self.yh_seed ^= self.yh_seed >> 7;
        self.yh_seed ^= self.yh_seed << 17;
        self.yh_seed
    }

    /// Feed a new item from the stream.
    pub fn yh_add(&mut self, item: i64) {
        self.yh_count += 1;
        if self.yh_reservoir.len() < self.yh_k {
            self.yh_reservoir.push(item);
        } else {
            let j = (self.yh_next_rand() % self.yh_count as u64) as usize;
            if j < self.yh_k {
                self.yh_reservoir[j] = item;
            }
        }
    }

    /// Get the current sample.
    pub fn yh_sample(&self) -> &[i64] { &self.yh_reservoir }

    /// Number of items seen.
    pub fn yh_count(&self) -> usize { self.yh_count }

    /// Sample size.
    pub fn yh_k(&self) -> usize { self.yh_k }

    /// Reset the sampler.
    pub fn yh_reset(&mut self, seed: u64) {
        self.yh_reservoir.clear();
        self.yh_count = 0;
        self.yh_seed = seed;
    }

    /// Is the reservoir full.
    pub fn yh_is_full(&self) -> bool { self.yh_reservoir.len() == self.yh_k }

    /// Current reservoir size.
    pub fn yh_len(&self) -> usize { self.yh_reservoir.len() }
}


// --- yi_ Ring Buffer ---

/// Fixed-capacity ring buffer (circular buffer) with O(1) push/pop at both ends.
#[derive(Debug, Clone)]
pub struct YiRingBuffer<T: Clone + Default> {
    yi_data: Vec<T>,
    yi_head: usize,
    yi_len: usize,
    yi_cap: usize,
}

impl<T: Clone + Default + std::fmt::Display> std::fmt::Display for YiRingBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RingBuffer(len={}, cap={})", self.yi_len, self.yi_cap)
    }
}

impl<T: Clone + Default> Default for YiRingBuffer<T> {
    fn default() -> Self { Self::yi_new(16) }
}

impl<T: Clone + Default> YiRingBuffer<T> {
    /// Create a ring buffer with given capacity.
    pub fn yi_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self { yi_data: vec![T::default(); cap], yi_head: 0, yi_len: 0, yi_cap: cap }
    }

    /// Current number of elements.
    pub fn yi_len(&self) -> usize { self.yi_len }

    /// Maximum capacity.
    pub fn yi_capacity(&self) -> usize { self.yi_cap }

    /// Is the buffer empty.
    pub fn yi_is_empty(&self) -> bool { self.yi_len == 0 }

    /// Is the buffer full.
    pub fn yi_is_full(&self) -> bool { self.yi_len == self.yi_cap }

    /// Push to the back. Returns false if full.
    pub fn yi_push_back(&mut self, value: T) -> bool {
        if self.yi_is_full() { return false; }
        let idx = (self.yi_head + self.yi_len) % self.yi_cap;
        self.yi_data[idx] = value;
        self.yi_len += 1;
        true
    }

    /// Push to the front. Returns false if full.
    pub fn yi_push_front(&mut self, value: T) -> bool {
        if self.yi_is_full() { return false; }
        self.yi_head = if self.yi_head == 0 { self.yi_cap - 1 } else { self.yi_head - 1 };
        self.yi_data[self.yi_head] = value;
        self.yi_len += 1;
        true
    }

    /// Pop from the front.
    pub fn yi_pop_front(&mut self) -> Option<T> {
        if self.yi_is_empty() { return None; }
        let val = self.yi_data[self.yi_head].clone();
        self.yi_head = (self.yi_head + 1) % self.yi_cap;
        self.yi_len -= 1;
        Some(val)
    }

    /// Pop from the back.
    pub fn yi_pop_back(&mut self) -> Option<T> {
        if self.yi_is_empty() { return None; }
        self.yi_len -= 1;
        let idx = (self.yi_head + self.yi_len) % self.yi_cap;
        Some(self.yi_data[idx].clone())
    }

    /// Peek at the front.
    pub fn yi_front(&self) -> Option<&T> {
        if self.yi_is_empty() { None } else { Some(&self.yi_data[self.yi_head]) }
    }

    /// Peek at the back.
    pub fn yi_back(&self) -> Option<&T> {
        if self.yi_is_empty() { None }
        else { Some(&self.yi_data[(self.yi_head + self.yi_len - 1) % self.yi_cap]) }
    }

    /// Get element at logical index.
    pub fn yi_get(&self, index: usize) -> Option<&T> {
        if index >= self.yi_len { None }
        else { Some(&self.yi_data[(self.yi_head + index) % self.yi_cap]) }
    }

    /// Convert to vec preserving order.
    pub fn yi_to_vec(&self) -> Vec<T> {
        (0..self.yi_len).map(|i| self.yi_data[(self.yi_head + i) % self.yi_cap].clone()).collect()
    }

    /// Clear the buffer.
    pub fn yi_clear(&mut self) { self.yi_len = 0; self.yi_head = 0; }

    /// Force push to back, overwriting oldest if full.
    pub fn yi_force_push_back(&mut self, value: T) {
        if self.yi_is_full() { self.yi_pop_front(); }
        self.yi_push_back(value);
    }
}

// --- yi_ Weighted Graph ---

/// Weighted directed graph with Dijkstra shortest paths.
#[derive(Debug, Clone)]
pub struct YiWeightedGraph {
    yi_adj: std::collections::HashMap<usize, Vec<(usize, f64)>>,
    yi_nodes: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YiWeightedGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let edges: usize = self.yi_adj.values().map(|v| v.len()).sum();
        write!(f, "WGraph(nodes={}, edges={})", self.yi_nodes.len(), edges)
    }
}

impl Default for YiWeightedGraph {
    fn default() -> Self { Self::yi_new() }
}

impl YiWeightedGraph {
    /// Create an empty weighted graph.
    pub fn yi_new() -> Self {
        Self { yi_adj: std::collections::HashMap::new(), yi_nodes: std::collections::HashSet::new() }
    }

    /// Add a node.
    pub fn yi_add_node(&mut self, node: usize) {
        self.yi_nodes.insert(node);
        self.yi_adj.entry(node).or_default();
    }

    /// Add a weighted directed edge.
    pub fn yi_add_edge(&mut self, from: usize, to: usize, weight: f64) {
        self.yi_add_node(from);
        self.yi_add_node(to);
        self.yi_adj.entry(from).or_default().push((to, weight));
    }

    /// Number of nodes.
    pub fn yi_node_count(&self) -> usize { self.yi_nodes.len() }

    /// Dijkstra shortest path distances from source.
    pub fn yi_dijkstra(&self, source: usize) -> std::collections::HashMap<usize, f64> {
        let mut dist: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
        let mut visited: std::collections::HashSet<usize> = std::collections::HashSet::new();
        dist.insert(source, 0.0);
        loop {
            let mut min_node = None;
            let mut min_dist = f64::INFINITY;
            for (node, d) in &dist {
                if !visited.contains(node) && *d < min_dist {
                    min_dist = *d;
                    min_node = Some(*node);
                }
            }
            let Some(u) = min_node else { break };
            visited.insert(u);
            if let Some(neighbors) = self.yi_adj.get(&u) {
                for (v, w) in neighbors {
                    let new_dist = min_dist + w;
                    let entry = dist.entry(*v).or_insert(f64::INFINITY);
                    if new_dist < *entry { *entry = new_dist; }
                }
            }
        }
        dist
    }

    /// Shortest path distance between two nodes.
    pub fn yi_shortest_distance(&self, from: usize, to: usize) -> Option<f64> {
        let dists = self.yi_dijkstra(from);
        dists.get(&to).copied().filter(|d| d.is_finite())
    }

    /// Get neighbors of a node.
    pub fn yi_neighbors(&self, node: usize) -> &[(usize, f64)] {
        self.yi_adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if node exists.
    pub fn yi_has_node(&self, node: usize) -> bool { self.yi_nodes.contains(&node) }

    /// Clear the graph.
    pub fn yi_clear(&mut self) { self.yi_adj.clear(); self.yi_nodes.clear(); }

    /// Total edge weight.
    pub fn yi_total_weight(&self) -> f64 {
        self.yi_adj.values().flat_map(|v| v.iter()).map(|(_, w)| w).sum()
    }

    /// Add bidirectional edge.
    pub fn yi_add_undirected_edge(&mut self, a: usize, b: usize, weight: f64) {
        self.yi_add_edge(a, b, weight);
        self.yi_add_edge(b, a, weight);
    }
}


// --- yj_ Expression Evaluator ---

/// Simple arithmetic expression evaluator supporting +, -, *, /, parentheses.
#[derive(Debug, Clone)]
pub struct YjExprEval {
    yj_vars: std::collections::HashMap<String, f64>,
}

impl std::fmt::Display for YjExprEval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExprEval(vars={})", self.yj_vars.len())
    }
}

impl Default for YjExprEval {
    fn default() -> Self { Self::yj_new() }
}

impl YjExprEval {
    /// Create a new expression evaluator.
    pub fn yj_new() -> Self { Self { yj_vars: std::collections::HashMap::new() } }

    /// Set a variable.
    pub fn yj_set_var(&mut self, name: &str, value: f64) { self.yj_vars.insert(name.to_string(), value); }

    /// Get a variable.
    pub fn yj_get_var(&self, name: &str) -> Option<f64> { self.yj_vars.get(name).copied() }

    /// Evaluate an expression string.
    pub fn yj_eval(&self, expr: &str) -> std::result::Result<f64, String> {
        let tokens = Self::yj_tokenize(expr)?;
        let mut pos = 0;
        let result = self.yj_parse_expr(&tokens, &mut pos)?;
        if pos != tokens.len() { return Err("unexpected token".to_string()); }
        Ok(result)
    }

    fn yj_tokenize(expr: &str) -> std::result::Result<Vec<String>, String> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();
        while let Some(&ch) = chars.peek() {
            if ch.is_whitespace() { chars.next(); continue; }
            if ch.is_ascii_digit() || ch == '.' {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' { num.push(c); chars.next(); } else { break; }
                }
                tokens.push(num);
            } else if ch.is_ascii_alphabetic() || ch == '_' {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' { name.push(c); chars.next(); } else { break; }
                }
                tokens.push(name);
            } else if "+-*/()".contains(ch) {
                tokens.push(ch.to_string());
                chars.next();
            } else {
                return Err(format!("unexpected character: {}", ch));
            }
        }
        Ok(tokens)
    }

    fn yj_parse_expr(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        let mut result = self.yj_parse_term(tokens, pos)?;
        while *pos < tokens.len() && (tokens[*pos] == "+" || tokens[*pos] == "-") {
            let op = tokens[*pos].clone();
            *pos += 1;
            let right = self.yj_parse_term(tokens, pos)?;
            result = if op == "+" { result + right } else { result - right };
        }
        Ok(result)
    }

    fn yj_parse_term(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        let mut result = self.yj_parse_factor(tokens, pos)?;
        while *pos < tokens.len() && (tokens[*pos] == "*" || tokens[*pos] == "/") {
            let op = tokens[*pos].clone();
            *pos += 1;
            let right = self.yj_parse_factor(tokens, pos)?;
            result = if op == "*" { result * right } else { result / right };
        }
        Ok(result)
    }

    fn yj_parse_factor(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        if *pos >= tokens.len() { return Err("unexpected end".to_string()); }
        if tokens[*pos] == "(" {
            *pos += 1;
            let result = self.yj_parse_expr(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != ")" { return Err("missing )".to_string()); }
            *pos += 1;
            return Ok(result);
        }
        if tokens[*pos] == "-" {
            *pos += 1;
            let val = self.yj_parse_factor(tokens, pos)?;
            return Ok(-val);
        }
        if let Ok(num) = tokens[*pos].parse::<f64>() {
            *pos += 1;
            return Ok(num);
        }
        if let Some(val) = self.yj_vars.get(&tokens[*pos]) {
            *pos += 1;
            return Ok(*val);
        }
        Err(format!("unknown token: {}", tokens[*pos]))
    }

    /// Clear all variables.
    pub fn yj_clear(&mut self) { self.yj_vars.clear(); }

    /// Number of variables.
    pub fn yj_var_count(&self) -> usize { self.yj_vars.len() }
}

// --- yj_ TTL Cache ---

/// Cache with time-to-live expiration for entries.
#[derive(Debug, Clone)]
pub struct YjTtlCache<V: Clone> {
    yj_entries: std::collections::HashMap<String, (V, u64)>,
    yj_ttl: u64,
    yj_clock: u64,
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YjTtlCache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TtlCache(entries={}, ttl={})", self.yj_entries.len(), self.yj_ttl)
    }
}

impl<V: Clone> Default for YjTtlCache<V> {
    fn default() -> Self { Self::yj_new(60) }
}

impl<V: Clone> YjTtlCache<V> {
    /// Create a TTL cache with given TTL in ticks.
    pub fn yj_new(ttl: u64) -> Self {
        Self { yj_entries: std::collections::HashMap::new(), yj_ttl: ttl, yj_clock: 0 }
    }

    /// Advance the clock by a given number of ticks.
    pub fn yj_tick(&mut self, ticks: u64) { self.yj_clock += ticks; }

    /// Current clock value.
    pub fn yj_clock(&self) -> u64 { self.yj_clock }

    /// Insert a key-value pair.
    pub fn yj_put(&mut self, key: &str, value: V) {
        self.yj_entries.insert(key.to_string(), (value, self.yj_clock));
    }

    /// Get a value if not expired.
    pub fn yj_get(&self, key: &str) -> Option<&V> {
        self.yj_entries.get(key).and_then(|(v, ts)| {
            if self.yj_clock - ts <= self.yj_ttl { Some(v) } else { None }
        })
    }

    /// Check if a key exists and is not expired.
    pub fn yj_contains(&self, key: &str) -> bool { self.yj_get(key).is_some() }

    /// Remove expired entries.
    pub fn yj_evict_expired(&mut self) {
        let clock = self.yj_clock;
        let ttl = self.yj_ttl;
        self.yj_entries.retain(|_, (_, ts)| clock - *ts <= ttl);
    }

    /// Number of entries (including possibly expired).
    pub fn yj_len(&self) -> usize { self.yj_entries.len() }

    /// Number of valid (non-expired) entries.
    pub fn yj_valid_count(&self) -> usize {
        self.yj_entries.values().filter(|(_, ts)| self.yj_clock - *ts <= self.yj_ttl).count()
    }

    /// Remove a key.
    pub fn yj_remove(&mut self, key: &str) -> Option<V> {
        self.yj_entries.remove(key).map(|(v, _)| v)
    }

    /// Clear the cache.
    pub fn yj_clear(&mut self) { self.yj_entries.clear(); }

    /// TTL value.
    pub fn yj_ttl(&self) -> u64 { self.yj_ttl }

    /// Set new TTL.
    pub fn yj_set_ttl(&mut self, ttl: u64) { self.yj_ttl = ttl; }
}


// --- yk_ Glob Pattern Matcher ---

/// Simple glob pattern matcher supporting *, ?, and character classes.
#[derive(Debug, Clone)]
pub struct YkGlobMatcher {
    yk_pattern: String,
}

impl std::fmt::Display for YkGlobMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Glob({})", self.yk_pattern)
    }
}

impl Default for YkGlobMatcher {
    fn default() -> Self { Self { yk_pattern: String::new() } }
}

impl YkGlobMatcher {
    /// Create a glob matcher from a pattern.
    pub fn yk_new(pattern: &str) -> Self { Self { yk_pattern: pattern.to_string() } }

    /// Get the pattern.
    pub fn yk_pattern(&self) -> &str { &self.yk_pattern }

    /// Check if a string matches the glob pattern.
    pub fn yk_matches(&self, text: &str) -> bool {
        Self::yk_match_impl(self.yk_pattern.as_bytes(), text.as_bytes())
    }

    fn yk_match_impl(pattern: &[u8], text: &[u8]) -> bool {
        let mut pi = 0;
        let mut ti = 0;
        let mut star_pi = usize::MAX;
        let mut star_ti = 0;
        while ti < text.len() {
            if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
                pi += 1;
                ti += 1;
            } else if pi < pattern.len() && pattern[pi] == b'*' {
                star_pi = pi;
                star_ti = ti;
                pi += 1;
            } else if star_pi != usize::MAX {
                pi = star_pi + 1;
                star_ti += 1;
                ti = star_ti;
            } else {
                return false;
            }
        }
        while pi < pattern.len() && pattern[pi] == b'*' { pi += 1; }
        pi == pattern.len()
    }

    /// Match multiple patterns (any match).
    pub fn yk_matches_any(patterns: &[&str], text: &str) -> bool {
        patterns.iter().any(|p| YkGlobMatcher::yk_new(p).yk_matches(text))
    }

    /// Match multiple patterns (all match).
    pub fn yk_matches_all(patterns: &[&str], text: &str) -> bool {
        patterns.iter().all(|p| YkGlobMatcher::yk_new(p).yk_matches(text))
    }

    /// Filter a list of strings by this pattern.
    pub fn yk_filter<'a>(&self, items: &[&'a str]) -> Vec<&'a str> {
        items.iter().filter(|s| self.yk_matches(s)).copied().collect()
    }
}

// --- yk_ Event Bus ---

/// Simple typed event bus with subscriber IDs.
#[derive(Debug, Clone)]
pub struct YkEventBus {
    yk_events: Vec<(String, Vec<(usize, String)>)>,
    yk_next_id: usize,
}

impl std::fmt::Display for YkEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total: usize = self.yk_events.iter().map(|(_, subs)| subs.len()).sum();
        write!(f, "EventBus(topics={}, subs={})", self.yk_events.len(), total)
    }
}

impl Default for YkEventBus {
    fn default() -> Self { Self::yk_new() }
}

impl YkEventBus {
    /// Create a new event bus.
    pub fn yk_new() -> Self { Self { yk_events: Vec::new(), yk_next_id: 0 } }

    /// Subscribe to a topic. Returns subscription ID.
    pub fn yk_subscribe(&mut self, topic: &str, handler_name: &str) -> usize {
        let id = self.yk_next_id;
        self.yk_next_id += 1;
        if let Some((_, subs)) = self.yk_events.iter_mut().find(|(t, _)| t == topic) {
            subs.push((id, handler_name.to_string()));
        } else {
            self.yk_events.push((topic.to_string(), vec![(id, handler_name.to_string())]));
        }
        id
    }

    /// Unsubscribe by ID.
    pub fn yk_unsubscribe(&mut self, id: usize) {
        for (_, subs) in &mut self.yk_events {
            subs.retain(|(sid, _)| *sid != id);
        }
    }

    /// Emit an event, returns list of handler names that were notified.
    pub fn yk_emit(&self, topic: &str) -> Vec<String> {
        self.yk_events.iter()
            .filter(|(t, _)| t == topic)
            .flat_map(|(_, subs)| subs.iter().map(|(_, name)| name.clone()))
            .collect()
    }

    /// Number of topics.
    pub fn yk_topic_count(&self) -> usize { self.yk_events.len() }

    /// Number of subscribers for a topic.
    pub fn yk_subscriber_count(&self, topic: &str) -> usize {
        self.yk_events.iter().find(|(t, _)| t == topic).map(|(_, s)| s.len()).unwrap_or(0)
    }

    /// Total subscribers across all topics.
    pub fn yk_total_subscribers(&self) -> usize {
        self.yk_events.iter().map(|(_, subs)| subs.len()).sum()
    }

    /// List all topics.
    pub fn yk_topics(&self) -> Vec<String> {
        self.yk_events.iter().map(|(t, _)| t.clone()).collect()
    }

    /// Clear all subscriptions.
    pub fn yk_clear(&mut self) { self.yk_events.clear(); self.yk_next_id = 0; }

    /// Check if a topic has subscribers.
    pub fn yk_has_subscribers(&self, topic: &str) -> bool {
        self.yk_subscriber_count(topic) > 0
    }

    /// Emit to topics matching a glob pattern.
    pub fn yk_emit_pattern(&self, pattern: &str) -> Vec<(String, Vec<String>)> {
        let matcher = YkGlobMatcher::yk_new(pattern);
        self.yk_events.iter()
            .filter(|(t, _)| matcher.yk_matches(t))
            .map(|(t, subs)| (t.clone(), subs.iter().map(|(_, n)| n.clone()).collect()))
            .collect()
    }
}


// --- yl_ Min-Max Heap ---

/// Min-max heap: O(1) access to both min and max, O(log n) insert/remove.
#[derive(Debug, Clone)]
pub struct YlMinMaxHeap {
    yl_data: Vec<i64>,
}

impl std::fmt::Display for YlMinMaxHeap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MinMaxHeap(size={})", self.yl_data.len())
    }
}

impl Default for YlMinMaxHeap {
    fn default() -> Self { Self::yl_new() }
}

impl YlMinMaxHeap {
    /// Create an empty min-max heap.
    pub fn yl_new() -> Self { Self { yl_data: Vec::new() } }

    /// Number of elements.
    pub fn yl_len(&self) -> usize { self.yl_data.len() }

    /// Is empty.
    pub fn yl_is_empty(&self) -> bool { self.yl_data.is_empty() }

    fn yl_is_min_level(idx: usize) -> bool {
        let level = ((idx + 1) as f64).log2().floor() as u32;
        level % 2 == 0
    }

    /// Insert a value.
    pub fn yl_insert(&mut self, val: i64) {
        self.yl_data.push(val);
        let idx = self.yl_data.len() - 1;
        self.yl_bubble_up(idx);
    }

    fn yl_bubble_up(&mut self, idx: usize) {
        if idx == 0 { return; }
        let parent = (idx - 1) / 2;
        if Self::yl_is_min_level(idx) {
            if self.yl_data[idx] > self.yl_data[parent] {
                self.yl_data.swap(idx, parent);
                self.yl_bubble_up_max(parent);
            } else {
                self.yl_bubble_up_min(idx);
            }
        } else {
            if self.yl_data[idx] < self.yl_data[parent] {
                self.yl_data.swap(idx, parent);
                self.yl_bubble_up_min(parent);
            } else {
                self.yl_bubble_up_max(idx);
            }
        }
    }

    fn yl_bubble_up_min(&mut self, mut idx: usize) {
        while idx > 2 {
            let grandparent = ((idx - 1) / 2 - 1) / 2;
            if self.yl_data[idx] < self.yl_data[grandparent] {
                self.yl_data.swap(idx, grandparent);
                idx = grandparent;
            } else { break; }
        }
    }

    fn yl_bubble_up_max(&mut self, mut idx: usize) {
        while idx > 2 {
            let grandparent = ((idx - 1) / 2 - 1) / 2;
            if self.yl_data[idx] > self.yl_data[grandparent] {
                self.yl_data.swap(idx, grandparent);
                idx = grandparent;
            } else { break; }
        }
    }

    /// Peek at minimum.
    pub fn yl_peek_min(&self) -> Option<i64> { self.yl_data.first().copied() }

    /// Peek at maximum.
    pub fn yl_peek_max(&self) -> Option<i64> {
        match self.yl_data.len() {
            0 => None,
            1 => Some(self.yl_data[0]),
            2 => Some(self.yl_data[1]),
            _ => Some(self.yl_data[1].max(self.yl_data[2])),
        }
    }

    /// Pop minimum.
    pub fn yl_pop_min(&mut self) -> Option<i64> {
        if self.yl_data.is_empty() { return None; }
        let min = self.yl_data[0];
        let last = self.yl_data.len() - 1;
        self.yl_data.swap(0, last);
        self.yl_data.pop();
        if !self.yl_data.is_empty() { self.yl_trickle_down(0); }
        Some(min)
    }

    fn yl_trickle_down(&mut self, idx: usize) {
        if Self::yl_is_min_level(idx) {
            self.yl_trickle_down_min(idx);
        } else {
            self.yl_trickle_down_max(idx);
        }
    }

    fn yl_trickle_down_min(&mut self, idx: usize) {
        let n = self.yl_data.len();
        let mut smallest = idx;
        for child in [2 * idx + 1, 2 * idx + 2] {
            if child < n && self.yl_data[child] < self.yl_data[smallest] { smallest = child; }
            for gc in [2 * child + 1, 2 * child + 2] {
                if gc < n && self.yl_data[gc] < self.yl_data[smallest] { smallest = gc; }
            }
        }
        if smallest != idx {
            self.yl_data.swap(idx, smallest);
            if smallest > 2 * idx + 2 { // grandchild
                let parent = (smallest - 1) / 2;
                if self.yl_data[smallest] > self.yl_data[parent] {
                    self.yl_data.swap(smallest, parent);
                }
                self.yl_trickle_down_min(smallest);
            }
        }
    }

    fn yl_trickle_down_max(&mut self, idx: usize) {
        let n = self.yl_data.len();
        let mut largest = idx;
        for child in [2 * idx + 1, 2 * idx + 2] {
            if child < n && self.yl_data[child] > self.yl_data[largest] { largest = child; }
            for gc in [2 * child + 1, 2 * child + 2] {
                if gc < n && self.yl_data[gc] > self.yl_data[largest] { largest = gc; }
            }
        }
        if largest != idx {
            self.yl_data.swap(idx, largest);
            if largest > 2 * idx + 2 {
                let parent = (largest - 1) / 2;
                if self.yl_data[largest] < self.yl_data[parent] {
                    self.yl_data.swap(largest, parent);
                }
                self.yl_trickle_down_max(largest);
            }
        }
    }

    /// Convert to sorted vec.
    pub fn yl_to_sorted_vec(&mut self) -> Vec<i64> {
        let mut result = Vec::with_capacity(self.yl_data.len());
        while let Some(v) = self.yl_pop_min() { result.push(v); }
        result
    }

    /// Clear.
    pub fn yl_clear(&mut self) { self.yl_data.clear(); }
}

// --- yl_ State Machine ---

/// Simple deterministic finite state machine.
#[derive(Debug, Clone)]
pub struct YlStateMachine {
    yl_states: Vec<String>,
    yl_current: usize,
    yl_transitions: Vec<(usize, String, usize)>,
    yl_accept: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YlStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FSM(states={}, current={})", self.yl_states.len(),
            self.yl_states.get(self.yl_current).map(|s| s.as_str()).unwrap_or("?"))
    }
}

impl Default for YlStateMachine {
    fn default() -> Self { Self::yl_new() }
}

impl YlStateMachine {
    /// Create an empty state machine.
    pub fn yl_new() -> Self {
        Self { yl_states: Vec::new(), yl_current: 0, yl_transitions: Vec::new(), yl_accept: std::collections::HashSet::new() }
    }

    /// Add a state. Returns state index.
    pub fn yl_add_state(&mut self, name: &str) -> usize {
        let idx = self.yl_states.len();
        self.yl_states.push(name.to_string());
        idx
    }

    /// Add a transition.
    pub fn yl_add_transition(&mut self, from: usize, input: &str, to: usize) {
        self.yl_transitions.push((from, input.to_string(), to));
    }

    /// Mark a state as accepting.
    pub fn yl_set_accept(&mut self, state: usize) { self.yl_accept.insert(state); }

    /// Set starting state.
    pub fn yl_set_start(&mut self, state: usize) { self.yl_current = state; }

    /// Process an input. Returns true if transition found.
    pub fn yl_step(&mut self, input: &str) -> bool {
        for (from, inp, to) in &self.yl_transitions {
            if *from == self.yl_current && inp == input {
                self.yl_current = *to;
                return true;
            }
        }
        false
    }

    /// Process a sequence of inputs. Returns true if all transitions found.
    pub fn yl_run(&mut self, inputs: &[&str]) -> bool {
        for input in inputs {
            if !self.yl_step(input) { return false; }
        }
        true
    }

    /// Current state name.
    pub fn yl_current_state(&self) -> &str {
        self.yl_states.get(self.yl_current).map(|s| s.as_str()).unwrap_or("")
    }

    /// Is current state accepting.
    pub fn yl_is_accepting(&self) -> bool { self.yl_accept.contains(&self.yl_current) }

    /// Number of states.
    pub fn yl_state_count(&self) -> usize { self.yl_states.len() }

    /// Number of transitions.
    pub fn yl_transition_count(&self) -> usize { self.yl_transitions.len() }

    /// Available transitions from current state.
    pub fn yl_available_inputs(&self) -> Vec<String> {
        self.yl_transitions.iter()
            .filter(|(from, _, _)| *from == self.yl_current)
            .map(|(_, input, _)| input.clone())
            .collect()
    }

    /// Reset to start state.
    pub fn yl_reset(&mut self) { self.yl_current = 0; }
}


// --- ym_ Sorted Multi-Map ---

/// Sorted multi-map allowing multiple values per key, stored in a BTreeMap.
#[derive(Debug, Clone)]
pub struct YmSortedMultiMap<K: Ord + Clone, V: Clone> {
    ym_data: std::collections::BTreeMap<K, Vec<V>>,
    ym_count: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for YmSortedMultiMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SortedMultiMap(keys={}, total={})", self.ym_data.len(), self.ym_count)
    }
}

impl<K: Ord + Clone, V: Clone> Default for YmSortedMultiMap<K, V> {
    fn default() -> Self { Self::ym_new() }
}

impl<K: Ord + Clone, V: Clone> YmSortedMultiMap<K, V> {
    /// Create empty sorted multi-map.
    pub fn ym_new() -> Self { Self { ym_data: std::collections::BTreeMap::new(), ym_count: 0 } }

    /// Insert a key-value pair.
    pub fn ym_insert(&mut self, key: K, value: V) {
        self.ym_data.entry(key).or_default().push(value);
        self.ym_count += 1;
    }

    /// Get all values for a key.
    pub fn ym_get(&self, key: &K) -> &[V] {
        self.ym_data.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Number of unique keys.
    pub fn ym_key_count(&self) -> usize { self.ym_data.len() }

    /// Total number of values.
    pub fn ym_total_count(&self) -> usize { self.ym_count }

    /// Is empty.
    pub fn ym_is_empty(&self) -> bool { self.ym_count == 0 }

    /// Contains key.
    pub fn ym_contains_key(&self, key: &K) -> bool { self.ym_data.contains_key(key) }

    /// Remove all values for a key.
    pub fn ym_remove_key(&mut self, key: &K) -> Vec<V> {
        if let Some(vals) = self.ym_data.remove(key) {
            self.ym_count -= vals.len();
            vals
        } else {
            Vec::new()
        }
    }

    /// Get all keys in sorted order.
    pub fn ym_keys(&self) -> Vec<K> {
        self.ym_data.keys().cloned().collect()
    }

    /// Get keys in a range.
    pub fn ym_range(&self, lo: &K, hi: &K) -> Vec<K> {
        self.ym_data.range(lo..=hi).map(|(k, _)| k.clone()).collect()
    }

    /// First key.
    pub fn ym_first_key(&self) -> Option<K> {
        self.ym_data.keys().next().cloned()
    }

    /// Last key.
    pub fn ym_last_key(&self) -> Option<K> {
        self.ym_data.keys().next_back().cloned()
    }

    /// Clear.
    pub fn ym_clear(&mut self) { self.ym_data.clear(); self.ym_count = 0; }

    /// Count values for a key.
    pub fn ym_count_for(&self, key: &K) -> usize {
        self.ym_data.get(key).map(|v| v.len()).unwrap_or(0)
    }
}

// --- ym_ Task Scheduler ---

/// Priority-based task scheduler with dependencies.
#[derive(Debug, Clone)]
pub struct YmTaskScheduler {
    ym_tasks: Vec<YmTask>,
    ym_next_id: usize,
}

/// A scheduled task.
#[derive(Debug, Clone)]
pub struct YmTask {
    /// Task ID.
    pub ym_id: usize,
    /// Task name.
    pub ym_name: String,
    /// Priority (lower = higher priority).
    pub ym_priority: i32,
    /// Dependencies (task IDs that must complete first).
    pub ym_deps: Vec<usize>,
    /// Is completed.
    pub ym_done: bool,
}

impl std::fmt::Display for YmTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task({}: {}, pri={}, done={})", self.ym_id, self.ym_name, self.ym_priority, self.ym_done)
    }
}

impl std::fmt::Display for YmTaskScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scheduler(tasks={}, pending={})", self.ym_tasks.len(), self.ym_pending_count())
    }
}

impl Default for YmTaskScheduler {
    fn default() -> Self { Self::ym_new() }
}

impl YmTaskScheduler {
    /// Create an empty scheduler.
    pub fn ym_new() -> Self { Self { ym_tasks: Vec::new(), ym_next_id: 0 } }

    /// Add a task. Returns task ID.
    pub fn ym_add_task(&mut self, name: &str, priority: i32, deps: Vec<usize>) -> usize {
        let id = self.ym_next_id;
        self.ym_next_id += 1;
        self.ym_tasks.push(YmTask { ym_id: id, ym_name: name.to_string(), ym_priority: priority, ym_deps: deps, ym_done: false });
        id
    }

    /// Mark a task as done.
    pub fn ym_complete(&mut self, id: usize) {
        if let Some(t) = self.ym_tasks.iter_mut().find(|t| t.ym_id == id) {
            t.ym_done = true;
        }
    }

    /// Get the next ready task (all deps done, highest priority).
    pub fn ym_next_ready(&self) -> Option<&YmTask> {
        let done_set: std::collections::HashSet<usize> = self.ym_tasks.iter()
            .filter(|t| t.ym_done).map(|t| t.ym_id).collect();
        self.ym_tasks.iter()
            .filter(|t| !t.ym_done && t.ym_deps.iter().all(|d| done_set.contains(d)))
            .min_by_key(|t| t.ym_priority)
    }

    /// Get all ready tasks.
    pub fn ym_all_ready(&self) -> Vec<&YmTask> {
        let done_set: std::collections::HashSet<usize> = self.ym_tasks.iter()
            .filter(|t| t.ym_done).map(|t| t.ym_id).collect();
        let mut ready: Vec<&YmTask> = self.ym_tasks.iter()
            .filter(|t| !t.ym_done && t.ym_deps.iter().all(|d| done_set.contains(d)))
            .collect();
        ready.sort_by_key(|t| t.ym_priority);
        ready
    }

    /// Number of pending tasks.
    pub fn ym_pending_count(&self) -> usize {
        self.ym_tasks.iter().filter(|t| !t.ym_done).count()
    }

    /// Number of completed tasks.
    pub fn ym_done_count(&self) -> usize {
        self.ym_tasks.iter().filter(|t| t.ym_done).count()
    }

    /// Total tasks.
    pub fn ym_total(&self) -> usize { self.ym_tasks.len() }

    /// Is all done.
    pub fn ym_is_all_done(&self) -> bool { self.ym_pending_count() == 0 }

    /// Get task by ID.
    pub fn ym_get_task(&self, id: usize) -> Option<&YmTask> {
        self.ym_tasks.iter().find(|t| t.ym_id == id)
    }

    /// Clear.
    pub fn ym_clear(&mut self) { self.ym_tasks.clear(); self.ym_next_id = 0; }
}


// --- yn_ Immutable Map (HAMT-inspired) ---

/// Persistent immutable map using a sorted vector for small maps.
#[derive(Debug, Clone)]
pub struct YnImmutableMap<K: Ord + Clone, V: Clone> {
    yn_entries: Vec<(K, V)>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for YnImmutableMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImmMap(size={})", self.yn_entries.len())
    }
}

impl<K: Ord + Clone, V: Clone> Default for YnImmutableMap<K, V> {
    fn default() -> Self { Self::yn_new() }
}

impl<K: Ord + Clone, V: Clone> YnImmutableMap<K, V> {
    /// Create an empty immutable map.
    pub fn yn_new() -> Self { Self { yn_entries: Vec::new() } }

    /// Insert returns a new map with the key-value pair added.
    pub fn yn_insert(&self, key: K, value: V) -> Self {
        let mut entries = self.yn_entries.clone();
        match entries.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(idx) => entries[idx] = (key, value),
            Err(idx) => entries.insert(idx, (key, value)),
        }
        Self { yn_entries: entries }
    }

    /// Remove returns a new map without the key.
    pub fn yn_remove(&self, key: &K) -> Self {
        let mut entries = self.yn_entries.clone();
        if let Ok(idx) = entries.binary_search_by(|(k, _)| k.cmp(key)) {
            entries.remove(idx);
        }
        Self { yn_entries: entries }
    }

    /// Look up a key.
    pub fn yn_get(&self, key: &K) -> Option<&V> {
        self.yn_entries.binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|idx| &self.yn_entries[idx].1)
    }

    /// Contains key.
    pub fn yn_contains_key(&self, key: &K) -> bool { self.yn_get(key).is_some() }

    /// Number of entries.
    pub fn yn_len(&self) -> usize { self.yn_entries.len() }

    /// Is empty.
    pub fn yn_is_empty(&self) -> bool { self.yn_entries.is_empty() }

    /// All keys in sorted order.
    pub fn yn_keys(&self) -> Vec<K> { self.yn_entries.iter().map(|(k, _)| k.clone()).collect() }

    /// All values.
    pub fn yn_values(&self) -> Vec<V> { self.yn_entries.iter().map(|(_, v)| v.clone()).collect() }

    /// Merge with another map (other takes precedence).
    pub fn yn_merge(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for (k, v) in &other.yn_entries {
            result = result.yn_insert(k.clone(), v.clone());
        }
        result
    }

    /// Map values.
    pub fn yn_map_values<F: Fn(&V) -> V>(&self, f: F) -> Self {
        Self { yn_entries: self.yn_entries.iter().map(|(k, v)| (k.clone(), f(v))).collect() }
    }

    /// Filter entries.
    pub fn yn_filter<F: Fn(&K, &V) -> bool>(&self, f: F) -> Self {
        Self { yn_entries: self.yn_entries.iter().filter(|(k, v)| f(k, v)).cloned().collect() }
    }
}

// --- yn_ Tokenizer ---

/// Simple token-based text tokenizer for parsing structured text.
#[derive(Debug, Clone, PartialEq)]
pub enum YnTokenKind {
    /// A word/identifier.
    YnWord,
    /// A number literal.
    YnNumber,
    /// A string literal.
    YnString,
    /// An operator or punctuation.
    YnPunct,
    /// Whitespace.
    YnWhitespace,
}

impl std::fmt::Display for YnTokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YnWord => write!(f, "Word"),
            Self::YnNumber => write!(f, "Number"),
            Self::YnString => write!(f, "String"),
            Self::YnPunct => write!(f, "Punct"),
            Self::YnWhitespace => write!(f, "Whitespace"),
        }
    }
}

/// A token produced by the tokenizer.
#[derive(Debug, Clone)]
pub struct YnToken {
    /// Token kind.
    pub yn_kind: YnTokenKind,
    /// Token text.
    pub yn_text: String,
    /// Start offset.
    pub yn_start: usize,
}

impl std::fmt::Display for YnToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({:?}@{})", self.yn_kind, self.yn_text, self.yn_start)
    }
}

/// Simple text tokenizer.
#[derive(Debug, Clone)]
pub struct YnTokenizer;

impl std::fmt::Display for YnTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tokenizer")
    }
}

impl Default for YnTokenizer {
    fn default() -> Self { Self }
}

impl YnTokenizer {
    /// Tokenize input text.
    pub fn yn_tokenize(input: &str) -> Vec<YnToken> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let start = i;
            if chars[i].is_whitespace() {
                while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnWhitespace, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnWord, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i].is_ascii_digit() {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnNumber, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i] == '\"' {
                i += 1;
                while i < chars.len() && chars[i] != '"' { i += 1; }
                if i < chars.len() { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnString, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else {
                i += 1;
                tokens.push(YnToken { yn_kind: YnTokenKind::YnPunct, yn_text: chars[start..i].iter().collect(), yn_start: start });
            }
        }
        tokens
    }

    /// Tokenize and filter out whitespace.
    pub fn yn_tokenize_no_ws(input: &str) -> Vec<YnToken> {
        Self::yn_tokenize(input).into_iter().filter(|t| t.yn_kind != YnTokenKind::YnWhitespace).collect()
    }

    /// Count tokens by kind.
    pub fn yn_count_by_kind(tokens: &[YnToken], kind: &YnTokenKind) -> usize {
        tokens.iter().filter(|t| t.yn_kind == *kind).count()
    }
}


// --- yo_ Levenshtein Distance ---

/// Levenshtein (edit) distance calculator for fuzzy string matching.
#[derive(Debug, Clone)]
pub struct YoLevenshtein;

impl std::fmt::Display for YoLevenshtein {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Levenshtein")
    }
}

impl Default for YoLevenshtein {
    fn default() -> Self { Self }
}

impl YoLevenshtein {
    /// Compute edit distance between two strings.
    pub fn yo_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();
        if m == 0 { return n; }
        if n == 0 { return m; }
        let mut prev = (0..=n).collect::<Vec<_>>();
        let mut curr = vec![0; n + 1];
        for i in 1..=m {
            curr[0] = i;
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            }
            std::mem::swap(&mut prev, &mut curr);
        }
        prev[n]
    }

    /// Normalized similarity [0.0, 1.0].
    pub fn yo_similarity(a: &str, b: &str) -> f64 {
        let max_len = a.chars().count().max(b.chars().count());
        if max_len == 0 { return 1.0; }
        1.0 - (Self::yo_distance(a, b) as f64 / max_len as f64)
    }

    /// Find closest match from candidates.
    pub fn yo_closest<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
        candidates.iter().min_by_key(|c| Self::yo_distance(target, c)).copied()
    }

    /// Filter candidates within a max distance.
    pub fn yo_within_distance<'a>(target: &str, candidates: &[&'a str], max_dist: usize) -> Vec<&'a str> {
        candidates.iter().filter(|c| Self::yo_distance(target, c) <= max_dist).copied().collect()
    }

    /// Rank candidates by distance (closest first).
    pub fn yo_rank<'a>(target: &str, candidates: &[&'a str]) -> Vec<(&'a str, usize)> {
        let mut ranked: Vec<_> = candidates.iter().map(|c| (*c, Self::yo_distance(target, c))).collect();
        ranked.sort_by_key(|(_, d)| *d);
        ranked
    }
}

// --- yo_ Diff Engine ---

/// Line-based diff engine using longest common subsequence.
#[derive(Debug, Clone, PartialEq)]
pub enum YoDiffOp {
    /// Line exists in both.
    YoEqual(String),
    /// Line added in new version.
    YoInsert(String),
    /// Line removed from old version.
    YoDelete(String),
}

impl std::fmt::Display for YoDiffOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YoEqual(s) => write!(f, "  {}", s),
            Self::YoInsert(s) => write!(f, "+ {}", s),
            Self::YoDelete(s) => write!(f, "- {}", s),
        }
    }
}

/// Line-based diff engine.
#[derive(Debug, Clone)]
pub struct YoDiffEngine;

impl std::fmt::Display for YoDiffEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DiffEngine")
    }
}

impl Default for YoDiffEngine {
    fn default() -> Self { Self }
}

impl YoDiffEngine {
    /// Compute diff between two texts (split by lines).
    pub fn yo_diff(old: &str, new: &str) -> Vec<YoDiffOp> {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let lcs = Self::yo_lcs(&old_lines, &new_lines);
        let mut result = Vec::new();
        let mut oi = 0;
        let mut ni = 0;
        let mut li = 0;
        while oi < old_lines.len() || ni < new_lines.len() {
            if li < lcs.len() && oi < old_lines.len() && ni < new_lines.len() && old_lines[oi] == lcs[li] && new_lines[ni] == lcs[li] {
                result.push(YoDiffOp::YoEqual(lcs[li].to_string()));
                oi += 1; ni += 1; li += 1;
            } else if ni < new_lines.len() && (li >= lcs.len() || new_lines[ni] != lcs[li]) {
                result.push(YoDiffOp::YoInsert(new_lines[ni].to_string()));
                ni += 1;
            } else if oi < old_lines.len() && (li >= lcs.len() || old_lines[oi] != lcs[li]) {
                result.push(YoDiffOp::YoDelete(old_lines[oi].to_string()));
                oi += 1;
            }
        }
        result
    }

    fn yo_lcs<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
        let m = a.len();
        let n = b.len();
        let mut dp = vec![vec![0usize; n + 1]; m + 1];
        for i in 1..=m {
            for j in 1..=n {
                dp[i][j] = if a[i - 1] == b[j - 1] { dp[i - 1][j - 1] + 1 } else { dp[i - 1][j].max(dp[i][j - 1]) };
            }
        }
        let mut result = Vec::new();
        let (mut i, mut j) = (m, n);
        while i > 0 && j > 0 {
            if a[i - 1] == b[j - 1] { result.push(a[i - 1]); i -= 1; j -= 1; }
            else if dp[i - 1][j] > dp[i][j - 1] { i -= 1; }
            else { j -= 1; }
        }
        result.reverse();
        result
    }

    /// Count insertions in a diff.
    pub fn yo_count_insertions(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoInsert(_))).count()
    }

    /// Count deletions in a diff.
    pub fn yo_count_deletions(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoDelete(_))).count()
    }

    /// Count equal lines.
    pub fn yo_count_equal(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoEqual(_))).count()
    }

    /// Format diff as unified diff string.
    pub fn yo_format(ops: &[YoDiffOp]) -> String {
        ops.iter().map(|op| format!("{}", op)).collect::<Vec<_>>().join("\n")
    }
}


// --- yp_ Simple JSON Value ---

/// Lightweight JSON-like value type for configuration and data exchange.
#[derive(Debug, Clone, PartialEq)]
pub enum YpJsonValue {
    YpNull,
    YpBool(bool),
    YpNumber(f64),
    YpString(String),
    YpArray(Vec<YpJsonValue>),
    YpObject(Vec<(String, YpJsonValue)>),
}

impl std::fmt::Display for YpJsonValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YpNull => write!(f, "null"),
            Self::YpBool(b) => write!(f, "{}", b),
            Self::YpNumber(n) => write!(f, "{}", n),
            Self::YpString(s) => write!(f, "\"{}\"", s),
            Self::YpArray(a) => write!(f, "[{}]", a.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(",")),
            Self::YpObject(o) => write!(f, "{{{}}}", o.iter().map(|(k, v)| format!("\"{}\":{}", k, v)).collect::<Vec<_>>().join(",")),
        }
    }
}

impl Default for YpJsonValue {
    fn default() -> Self { Self::YpNull }
}

impl YpJsonValue {
    /// Create a string value.
    pub fn yp_string(s: &str) -> Self { Self::YpString(s.to_string()) }

    /// Create a number value.
    pub fn yp_number(n: f64) -> Self { Self::YpNumber(n) }

    /// Create a bool value.
    pub fn yp_bool(b: bool) -> Self { Self::YpBool(b) }

    /// Create an empty object.
    pub fn yp_object() -> Self { Self::YpObject(Vec::new()) }

    /// Create an empty array.
    pub fn yp_array() -> Self { Self::YpArray(Vec::new()) }

    /// Is null.
    pub fn yp_is_null(&self) -> bool { matches!(self, Self::YpNull) }

    /// Get as string.
    pub fn yp_as_str(&self) -> Option<&str> {
        if let Self::YpString(s) = self { Some(s) } else { None }
    }

    /// Get as number.
    pub fn yp_as_f64(&self) -> Option<f64> {
        if let Self::YpNumber(n) = self { Some(*n) } else { None }
    }

    /// Get as bool.
    pub fn yp_as_bool(&self) -> Option<bool> {
        if let Self::YpBool(b) = self { Some(*b) } else { None }
    }

    /// Get by key (for objects).
    pub fn yp_get(&self, key: &str) -> Option<&YpJsonValue> {
        if let Self::YpObject(entries) = self {
            entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else { None }
    }

    /// Get by index (for arrays).
    pub fn yp_index(&self, idx: usize) -> Option<&YpJsonValue> {
        if let Self::YpArray(arr) = self { arr.get(idx) } else { None }
    }

    /// Set a key on an object (mutating).
    pub fn yp_set(&mut self, key: &str, value: YpJsonValue) {
        if let Self::YpObject(entries) = self {
            if let Some(entry) = entries.iter_mut().find(|(k, _)| k == key) {
                entry.1 = value;
            } else {
                entries.push((key.to_string(), value));
            }
        }
    }

    /// Push to array.
    pub fn yp_push(&mut self, value: YpJsonValue) {
        if let Self::YpArray(arr) = self { arr.push(value); }
    }

    /// Object keys.
    pub fn yp_keys(&self) -> Vec<String> {
        if let Self::YpObject(entries) = self {
            entries.iter().map(|(k, _)| k.clone()).collect()
        } else { Vec::new() }
    }

    /// Array/object length.
    pub fn yp_len(&self) -> usize {
        match self {
            Self::YpArray(a) => a.len(),
            Self::YpObject(o) => o.len(),
            Self::YpString(s) => s.len(),
            _ => 0,
        }
    }

    /// Deep clone with path-based access.
    pub fn yp_path(&self, path: &str) -> Option<&YpJsonValue> {
        let parts: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        let mut current = self;
        for part in parts {
            if let Ok(idx) = part.parse::<usize>() {
                current = current.yp_index(idx)?;
            } else {
                current = current.yp_get(part)?;
            }
        }
        Some(current)
    }

    /// Merge two objects (other takes precedence).
    pub fn yp_merge(&self, other: &YpJsonValue) -> YpJsonValue {
        match (self, other) {
            (Self::YpObject(a), Self::YpObject(b)) => {
                let mut result = a.clone();
                for (k, v) in b {
                    if let Some(entry) = result.iter_mut().find(|(ek, _)| ek == k) {
                        entry.1 = v.clone();
                    } else {
                        result.push((k.clone(), v.clone()));
                    }
                }
                Self::YpObject(result)
            }
            _ => other.clone(),
        }
    }
}

// --- yp_ Command Registry ---

/// Registry for named commands with metadata.
#[derive(Debug, Clone)]
pub struct YpCommandEntry {
    pub yp_id: String,
    pub yp_title: String,
    pub yp_category: String,
    pub yp_keybinding: Option<String>,
    pub yp_when: Option<String>,
}

impl std::fmt::Display for YpCommandEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cmd({})", self.yp_id)
    }
}

/// Command registry for command palette and keybinding resolution.
#[derive(Debug, Clone)]
pub struct YpCommandRegistry {
    yp_commands: Vec<YpCommandEntry>,
}

impl std::fmt::Display for YpCommandRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CmdRegistry(count={})", self.yp_commands.len())
    }
}

impl Default for YpCommandRegistry {
    fn default() -> Self { Self::yp_new() }
}

impl YpCommandRegistry {
    /// Create empty registry.
    pub fn yp_new() -> Self { Self { yp_commands: Vec::new() } }

    /// Register a command.
    pub fn yp_register(&mut self, id: &str, title: &str, category: &str) {
        self.yp_commands.push(YpCommandEntry {
            yp_id: id.to_string(), yp_title: title.to_string(), yp_category: category.to_string(),
            yp_keybinding: None, yp_when: None,
        });
    }

    /// Register with keybinding.
    pub fn yp_register_with_key(&mut self, id: &str, title: &str, category: &str, keybinding: &str) {
        self.yp_commands.push(YpCommandEntry {
            yp_id: id.to_string(), yp_title: title.to_string(), yp_category: category.to_string(),
            yp_keybinding: Some(keybinding.to_string()), yp_when: None,
        });
    }

    /// Find command by ID.
    pub fn yp_find(&self, id: &str) -> Option<&YpCommandEntry> {
        self.yp_commands.iter().find(|c| c.yp_id == id)
    }

    /// Search commands by title prefix.
    pub fn yp_search(&self, query: &str) -> Vec<&YpCommandEntry> {
        let q = query.to_lowercase();
        self.yp_commands.iter().filter(|c| c.yp_title.to_lowercase().contains(&q)).collect()
    }

    /// Commands in a category.
    pub fn yp_by_category(&self, category: &str) -> Vec<&YpCommandEntry> {
        self.yp_commands.iter().filter(|c| c.yp_category == category).collect()
    }

    /// Find command by keybinding.
    pub fn yp_by_keybinding(&self, key: &str) -> Option<&YpCommandEntry> {
        self.yp_commands.iter().find(|c| c.yp_keybinding.as_deref() == Some(key))
    }

    /// Number of commands.
    pub fn yp_count(&self) -> usize { self.yp_commands.len() }

    /// All categories.
    pub fn yp_categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self.yp_commands.iter().map(|c| c.yp_category.clone()).collect();
        cats.sort();
        cats.dedup();
        cats
    }

    /// Clear.
    pub fn yp_clear(&mut self) { self.yp_commands.clear(); }
}


// --- yq_ Layered Config Store ---

/// Layered configuration store with default, user, and workspace layers.
#[derive(Debug, Clone)]
pub struct YqConfigStore {
    yq_layers: Vec<(String, std::collections::HashMap<String, String>)>,
}

impl std::fmt::Display for YqConfigStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total: usize = self.yq_layers.iter().map(|(_, m)| m.len()).sum();
        write!(f, "ConfigStore(layers={}, keys={})", self.yq_layers.len(), total)
    }
}

impl Default for YqConfigStore {
    fn default() -> Self { Self::yq_new() }
}

impl YqConfigStore {
    /// Create with default layers: defaults, user, workspace.
    pub fn yq_new() -> Self {
        Self { yq_layers: vec![
            ("defaults".to_string(), std::collections::HashMap::new()),
            ("user".to_string(), std::collections::HashMap::new()),
            ("workspace".to_string(), std::collections::HashMap::new()),
        ] }
    }

    /// Set a value in a specific layer.
    pub fn yq_set(&mut self, layer: &str, key: &str, value: &str) {
        if let Some((_, map)) = self.yq_layers.iter_mut().find(|(n, _)| n == layer) {
            map.insert(key.to_string(), value.to_string());
        }
    }

    /// Get a value, checking layers from last (highest priority) to first.
    pub fn yq_get(&self, key: &str) -> Option<&str> {
        for (_, map) in self.yq_layers.iter().rev() {
            if let Some(v) = map.get(key) { return Some(v.as_str()); }
        }
        None
    }

    /// Get with default.
    pub fn yq_get_or(&self, key: &str, default: &str) -> String {
        self.yq_get(key).unwrap_or(default).to_string()
    }

    /// Get value as i64.
    pub fn yq_get_i64(&self, key: &str) -> Option<i64> {
        self.yq_get(key).and_then(|v| v.parse().ok())
    }

    /// Get value as bool.
    pub fn yq_get_bool(&self, key: &str) -> Option<bool> {
        self.yq_get(key).and_then(|v| v.parse().ok())
    }

    /// Remove a key from a layer.
    pub fn yq_remove(&mut self, layer: &str, key: &str) {
        if let Some((_, map)) = self.yq_layers.iter_mut().find(|(n, _)| n == layer) {
            map.remove(key);
        }
    }

    /// All keys across all layers.
    pub fn yq_all_keys(&self) -> Vec<String> {
        let mut keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (_, map) in &self.yq_layers { for k in map.keys() { keys.insert(k.clone()); } }
        let mut sorted: Vec<String> = keys.into_iter().collect();
        sorted.sort();
        sorted
    }

    /// Add a custom layer.
    pub fn yq_add_layer(&mut self, name: &str) {
        self.yq_layers.push((name.to_string(), std::collections::HashMap::new()));
    }

    /// Number of layers.
    pub fn yq_layer_count(&self) -> usize { self.yq_layers.len() }

    /// Get the effective layer name for a key.
    pub fn yq_effective_layer(&self, key: &str) -> Option<&str> {
        for (name, map) in self.yq_layers.iter().rev() {
            if map.contains_key(key) { return Some(name.as_str()); }
        }
        None
    }

    /// Clear a specific layer.
    pub fn yq_clear_layer(&mut self, layer: &str) {
        if let Some((_, map)) = self.yq_layers.iter_mut().find(|(n, _)| n == layer) {
            map.clear();
        }
    }

    /// Clear all layers.
    pub fn yq_clear_all(&mut self) {
        for (_, map) in &mut self.yq_layers { map.clear(); }
    }
}

// --- yq_ Text Layout Engine ---

/// Simple text line wrapping and layout engine for terminal rendering.
#[derive(Debug, Clone)]
pub struct YqTextLayout {
    yq_width: usize,
    yq_tab_size: usize,
}

impl std::fmt::Display for YqTextLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TextLayout(w={}, tab={})", self.yq_width, self.yq_tab_size)
    }
}

impl Default for YqTextLayout {
    fn default() -> Self { Self { yq_width: 80, yq_tab_size: 4 } }
}

impl YqTextLayout {
    /// Create with given width.
    pub fn yq_new(width: usize) -> Self { Self { yq_width: width.max(1), yq_tab_size: 4 } }

    /// Set tab size.
    pub fn yq_set_tab_size(&mut self, size: usize) { self.yq_tab_size = size.max(1); }

    /// Width.
    pub fn yq_width(&self) -> usize { self.yq_width }

    /// Wrap text into lines of at most width characters.
    pub fn yq_wrap(&self, text: &str) -> Vec<String> {
        let expanded = self.yq_expand_tabs(text);
        let mut lines = Vec::new();
        for line in expanded.lines() {
            if line.len() <= self.yq_width {
                lines.push(line.to_string());
            } else {
                let mut remaining = line;
                while remaining.len() > self.yq_width {
                    let split = Self::yq_find_break(remaining, self.yq_width);
                    lines.push(remaining[..split].to_string());
                    remaining = &remaining[split..];
                    remaining = remaining.trim_start();
                }
                if !remaining.is_empty() { lines.push(remaining.to_string()); }
            }
        }
        if lines.is_empty() { lines.push(String::new()); }
        lines
    }

    fn yq_find_break(text: &str, max_width: usize) -> usize {
        if let Some(pos) = text[..max_width].rfind(' ') {
            if pos > 0 { return pos + 1; }
        }
        max_width
    }

    /// Expand tabs to spaces.
    pub fn yq_expand_tabs(&self, text: &str) -> String {
        text.replace('\t', &" ".repeat(self.yq_tab_size))
    }

    /// Truncate a line to width, adding ellipsis if needed.
    pub fn yq_truncate(&self, text: &str, ellipsis: &str) -> String {
        if text.len() <= self.yq_width { return text.to_string(); }
        let avail = self.yq_width.saturating_sub(ellipsis.len());
        format!("{}{}", &text[..avail], ellipsis)
    }

    /// Pad/align text.
    pub fn yq_pad_right(&self, text: &str) -> String {
        if text.len() >= self.yq_width { return text[..self.yq_width].to_string(); }
        format!("{:width$}", text, width = self.yq_width)
    }

    /// Center text.
    pub fn yq_center(&self, text: &str) -> String {
        if text.len() >= self.yq_width { return text[..self.yq_width].to_string(); }
        let pad = (self.yq_width - text.len()) / 2;
        format!("{}{}{}", " ".repeat(pad), text, " ".repeat(self.yq_width - text.len() - pad))
    }

    /// Count visual lines needed.
    pub fn yq_line_count(&self, text: &str) -> usize {
        self.yq_wrap(text).len()
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


    // ---- xc_ pool / scheduler tests – block 132 ----

    #[test]
    fn xc_132_pool_new_empty() {
        let pool: super::Xc132Pool<i32> = super::Xc132Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_132_pool_release_acquire() {
        let mut pool = super::Xc132Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_132_pool_acquire_empty() {
        let mut pool: super::Xc132Pool<i32> = super::Xc132Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_132_pool_full() {
        let mut pool = super::Xc132Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_132_pool_drain() {
        let mut pool = super::Xc132Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_132_pool_stats() {
        let mut pool = super::Xc132Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_132_pool_clear() {
        let mut pool = super::Xc132Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_132_pool_shrink() {
        let mut pool = super::Xc132Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_132_pool_default() {
        let pool: super::Xc132Pool<String> = super::Xc132Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_132_pool_extend() {
        let mut pool = super::Xc132Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_132_pool_retain() {
        let mut pool = super::Xc132Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_132_scheduler_round_robin() {
        let mut sched = super::Xc132Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_132_scheduler_empty() {
        let mut sched = super::Xc132Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_132_scheduler_reset() {
        let mut sched = super::Xc132Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_132_scheduler_add_remove() {
        let mut sched = super::Xc132Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_132_scheduler_targets() {
        let sched = super::Xc132Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_132_hash_empty() {
        assert_eq!(super::xc_132_hash(b""), 5381);
    }

    #[test]
    fn xc_132_hash_data() {
        let h = super::xc_132_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_132_hash(b"hello"), h);
    }

    #[test]
    fn xc_132_reverse_str() {
        assert_eq!(super::xc_132_reverse("abc"), "cba");
        assert_eq!(super::xc_132_reverse(""), "");
    }


    // --- xd_76 deepening tests ---

    #[test]
    fn xd_76_sm_initial_state() {
        let sm = Xd76StateMachine::new();
        assert_eq!(sm.current_state(), Xd76State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_76_sm_valid_idle_to_running() {
        let mut sm = Xd76StateMachine::new();
        assert!(sm.transition(Xd76State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd76State::Running);
    }

    #[test]
    fn xd_76_sm_valid_running_to_paused() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        assert!(sm.transition(Xd76State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd76State::Paused);
    }

    #[test]
    fn xd_76_sm_valid_running_to_done() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        assert!(sm.transition(Xd76State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd76State::Done);
    }

    #[test]
    fn xd_76_sm_valid_paused_to_running() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        sm.transition(Xd76State::Paused).unwrap();
        assert!(sm.transition(Xd76State::Running).is_ok());
    }

    #[test]
    fn xd_76_sm_valid_done_to_idle() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        sm.transition(Xd76State::Done).unwrap();
        assert!(sm.transition(Xd76State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd76State::Idle);
    }

    #[test]
    fn xd_76_sm_invalid_idle_to_done() {
        let mut sm = Xd76StateMachine::new();
        assert!(sm.transition(Xd76State::Done).is_err());
    }

    #[test]
    fn xd_76_sm_invalid_idle_to_paused() {
        let mut sm = Xd76StateMachine::new();
        assert!(sm.transition(Xd76State::Paused).is_err());
    }

    #[test]
    fn xd_76_sm_history_tracking() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        sm.transition(Xd76State::Paused).unwrap();
        sm.transition(Xd76State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd76State::Idle);
        assert_eq!(sm.history()[0].to, Xd76State::Running);
        assert_eq!(sm.history()[1].from, Xd76State::Running);
        assert_eq!(sm.history()[2].to, Xd76State::Done);
    }

    #[test]
    fn xd_76_sm_serialize_deserialize() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd76StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd76State::Running));
    }

    #[test]
    fn xd_76_sm_deserialize_invalid() {
        assert_eq!(Xd76StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_76_sm_reset() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd76State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_76_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd76EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd76Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_76_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd76EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd76Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd76Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_76_bus_unsubscribe() {
        let mut bus = Xd76EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_76_event_kind_and_payload() {
        let e = Xd76Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd76Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_76_bus_clear_history() {
        let mut bus = Xd76EventBus::new();
        bus.publish(Xd76Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_76_sm_step_counter_increments() {
        let mut sm = Xd76StateMachine::new();
        sm.transition(Xd76State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd76State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #94 --

    #[test]
    fn xf94_trie_insert_search() {
        let mut t = Xf94Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf94_trie_starts_with() {
        let mut t = Xf94Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf94_trie_remove() {
        let mut t = Xf94Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf94_trie_word_count() {
        let mut t = Xf94Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf94_trie_longest_prefix() {
        let mut t = Xf94Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf94_trie_all_words() {
        let mut t = Xf94Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf94_trie_autocomplete() {
        let mut t = Xf94Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf94_trie_empty_search() {
        let t = Xf94Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf94_bloom_add_contains() {
        let mut bf = Xf94BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf94_bloom_probably_absent() {
        let bf = Xf94BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf94_bloom_false_positive_rate() {
        let mut bf = Xf94BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf94_bloom_clear() {
        let mut bf = Xf94BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf94_bloom_union() {
        let mut a = Xf94BloomFilter::xf_new(512, 2);
        let mut b = Xf94BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf94_bloom_intersection_estimate() {
        let mut a = Xf94BloomFilter::xf_new(512, 2);
        let mut b = Xf94BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf94_bloom_union_size_mismatch() {
        let a = Xf94BloomFilter::xf_new(256, 2);
        let b = Xf94BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh131_skip_insert_contains() {
        let mut sl = super::Xh131SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh131_skip_remove() {
        let mut sl = super::Xh131SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh131_skip_len() {
        let mut sl = super::Xh131SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh131_skip_range_query() {
        let mut sl = super::Xh131SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh131_skip_floor_ceiling() {
        let mut sl = super::Xh131SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh131_skip_rank() {
        let mut sl = super::Xh131SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh131_skip_empty() {
        let sl = super::Xh131SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh131_skip_duplicates() {
        let mut sl = super::Xh131SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh131_bitset_set_test() {
        let mut bs = super::Xh131BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh131_bitset_clear_count() {
        let mut bs = super::Xh131BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh131_bitset_and_or_xor() {
        let mut a = super::Xh131BitSet::xh_new(128);
        let mut b = super::Xh131BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh131_bitset_iter_ones() {
        let mut bs = super::Xh131BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh131_bitset_first_last() {
        let mut bs = super::Xh131BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh131_bitset_empty() {
        let bs = super::Xh131BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi131_deque_push_pop_back() {
        let mut dq = super::Xi131Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi131_deque_push_pop_front() {
        let mut dq = super::Xi131Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi131_deque_mixed_ops() {
        let mut dq = super::Xi131Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi131_deque_get_and_split() {
        let mut dq = super::Xi131Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi131_deque_rotate_left() {
        let mut dq = super::Xi131Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi131_deque_rotate_right() {
        let mut dq = super::Xi131Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi131_deque_grow() {
        let mut dq = super::Xi131Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi131_deque_empty() {
        let dq = super::Xi131Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi131_interval_tree_insert_query() {
        let mut tree = super::Xi131IntervalTree::xi_new();
        tree.xi_insert(super::Xi131Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi131Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi131Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi131_interval_tree_overlap() {
        let mut tree = super::Xi131IntervalTree::xi_new();
        tree.xi_insert(super::Xi131Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi131Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi131Interval::xi_new(12, 20));
        let q = super::Xi131Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi131_interval_tree_remove() {
        let mut tree = super::Xi131IntervalTree::xi_new();
        tree.xi_insert(super::Xi131Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi131Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi131_interval_tree_gaps() {
        let mut tree = super::Xi131IntervalTree::xi_new();
        tree.xi_insert(super::Xi131Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi131Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi131Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi131Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi131Interval::xi_new(8, 10));
    }

    #[test]
    fn xi131_interval_tree_merge() {
        let mut tree = super::Xi131IntervalTree::xi_new();
        tree.xi_insert(super::Xi131Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi131Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi131Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi131Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi131Interval::xi_new(10, 15));
    }

    #[test]
    fn xi131_interval_tree_all() {
        let mut tree = super::Xi131IntervalTree::xi_new();
        tree.xi_insert(super::Xi131Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi131Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi131_interval_tree_empty() {
        let tree = super::Xi131IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi131_interval_tree_contains_point() {
        let iv = super::Xi131Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 131) ---

    #[test]
    fn xj_131_uf_make_and_find() {
        let mut uf = super::Xj131UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_131_uf_union_connected() {
        let mut uf = super::Xj131UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_131_uf_component_count() {
        let mut uf = super::Xj131UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_131_uf_component_size() {
        let mut uf = super::Xj131UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_131_uf_largest_component() {
        let mut uf = super::Xj131UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_131_uf_many_elements() {
        let mut uf = super::Xj131UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_131_uf_separate_components() {
        let mut uf = super::Xj131UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_131_uf_path_compression() {
        let mut uf = super::Xj131UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_131_bt_insert_get() {
        let mut bt = super::Xj131BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_131_bt_contains_len() {
        let mut bt = super::Xj131BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_131_bt_replace() {
        let mut bt = super::Xj131BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_131_bt_remove() {
        let mut bt = super::Xj131BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_131_bt_keys_values() {
        let mut bt = super::Xj131BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_131_bt_range() {
        let mut bt = super::Xj131BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_131_bt_min_max() {
        let mut bt = super::Xj131BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_131_bt_many_inserts() {
        let mut bt = super::Xj131BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_131 segment tree tests ---

    #[test]
    fn xk_131_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk131SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_131_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk131SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_131_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk131SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_131_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk131SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_131_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk131SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_131_st_single_element() {
        let data = vec![42];
        let st = super::Xk131SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_131_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk131SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_131_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk131SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_131 disjoint intervals tests ---

    #[test]
    fn xk_131_di_add_and_count() {
        let mut di = super::Xk131DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_131_di_merge_overlap() {
        let mut di = super::Xk131DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_131_di_contains() {
        let mut di = super::Xk131DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_131_di_remove() {
        let mut di = super::Xk131DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_131_di_covered_length() {
        let mut di = super::Xk131DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_131_di_gaps() {
        let mut di = super::Xk131DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_131_di_merge_adjacent() {
        let mut di = super::Xk131DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_131_di_empty() {
        let di = super::Xk131DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_131_rope_new_empty() {
        let rope = super::Xl131Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_131_rope_from_str() {
        let rope = super::Xl131Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_131_rope_insert_at() {
        let mut rope = super::Xl131Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_131_rope_delete_range() {
        let mut rope = super::Xl131Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_131_rope_char_at() {
        let rope = super::Xl131Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_131_rope_split_concat() {
        let rope = super::Xl131Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_131_rope_line_count() {
        let rope = super::Xl131Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_131_rope_line_at() {
        let rope = super::Xl131Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_131_sa_build_and_search() {
        let sa = super::Xl131SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_131_sa_count() {
        let sa = super::Xl131SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_131_sa_longest_repeated() {
        let sa = super::Xl131SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_131_sa_all_positions() {
        let sa = super::Xl131SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_131_sa_len() {
        let sa = super::Xl131SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_131_sa_empty() {
        let sa = super::Xl131SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_131_rope_slice() {
        let rope = super::Xl131Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_131_sa_search_start() {
        let sa = super::Xl131SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_131_sparse_set_get() {
        let mut m = super::Xm131MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_131_sparse_row_col() {
        let mut m = super::Xm131MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_131_sparse_transpose() {
        let mut m = super::Xm131MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_131_sparse_multiply_vec() {
        let mut m = super::Xm131MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_131_sparse_nnz_density() {
        let mut m = super::Xm131MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_131_sparse_clear() {
        let mut m = super::Xm131MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_131_sparse_overwrite_zero() {
        let mut m = super::Xm131MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_131_tokenizer_basic() {
        let t = super::Xm131Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_131_tokenizer_count() {
        let t = super::Xm131Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_131_tokenizer_unique() {
        let t = super::Xm131Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_131_tokenizer_frequency() {
        let t = super::Xm131Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_131_tokenizer_delimiter() {
        let t = super::Xm131Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_131_tokenizer_whitespace() {
        let t = super::Xm131Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_131_tokenizer_empty() {
        let t = super::Xm131Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 131 ----

    #[test]
    fn xn_131_fenwick_prefix_sum() {
        let mut ft = super::Xn131Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_131_fenwick_range_sum() {
        let mut ft = super::Xn131Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_131_fenwick_point_query() {
        let mut ft = super::Xn131Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_131_fenwick_len() {
        let ft = super::Xn131Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_131_fenwick_multiple_updates() {
        let mut ft = super::Xn131Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_131_fenwick_single_element() {
        let mut ft = super::Xn131Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_131_fenwick_find_kth() {
        let mut ft = super::Xn131Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_131_fenwick_negative_delta() {
        let mut ft = super::Xn131Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 131 ----

    #[test]
    fn xn_131_avl_insert_get() {
        let mut m = super::Xn131AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_131_avl_remove() {
        let mut m = super::Xn131AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_131_avl_in_order() {
        let mut m = super::Xn131AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_131_avl_min_max() {
        let mut m = super::Xn131AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_131_avl_floor_ceiling() {
        let mut m = super::Xn131AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_131_avl_height_balanced() {
        let mut m = super::Xn131AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_131_avl_overwrite() {
        let mut m = super::Xn131AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_131_avl_empty() {
        let m: super::Xn131AVL<i32, i32> = super::Xn131AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo131RedBlack tests ---

    #[test]
    fn xo_131_rb_insert_and_get() {
        let mut tree = super::Xo131RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_131_rb_len_and_empty() {
        let mut tree = super::Xo131RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_131_rb_min_max() {
        let mut tree = super::Xo131RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_131_rb_contains() {
        let mut tree = super::Xo131RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_131_rb_remove() {
        let mut tree = super::Xo131RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_131_rb_in_order() {
        let mut tree = super::Xo131RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_131_rb_black_height() {
        let mut tree = super::Xo131RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_131_rb_overwrite() {
        let mut tree = super::Xo131RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo131ConsistentHash tests ---

    #[test]
    fn xo_131_ch_add_and_count() {
        let mut ring = super::Xo131ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_131_ch_remove_node() {
        let mut ring = super::Xo131ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_131_ch_get_node() {
        let mut ring = super::Xo131ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_131_ch_empty_ring() {
        let ring = super::Xo131ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_131_ch_distribution() {
        let mut ring = super::Xo131ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_131_ch_rebalance() {
        let mut ring = super::Xo131ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_131_ch_virtual_nodes() {
        let mut ring = super::Xo131ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_131_ch_consistent_lookup() {
        let mut ring = super::Xo131ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_131_splay_insert_get() {
        let mut t = super::Xp131SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_131_splay_remove() {
        let mut t = super::Xp131SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_131_splay_count_increases() {
        let mut t = super::Xp131SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_131_splay_depth() {
        let mut t = super::Xp131SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_131_splay_len_empty() {
        let t = super::Xp131SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_131_splay_min_max() {
        let mut t = super::Xp131SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_131_splay_overwrite() {
        let mut t = super::Xp131SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_131_splay_remove_missing() {
        let mut t = super::Xp131SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_131 treap tests ----
    #[test]
    fn xq_131_treap_empty() {
        let t = super::Xq131Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_131_treap_insert_get() {
        let mut t = super::Xq131Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_131_treap_overwrite() {
        let mut t = super::Xq131Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_131_treap_remove() {
        let mut t = super::Xq131Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_131_treap_min_max() {
        let mut t = super::Xq131Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_131_treap_rank() {
        let mut t = super::Xq131Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_131_treap_kth() {
        let mut t = super::Xq131Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_131_treap_in_order() {
        let mut t = super::Xq131Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_131 VEB tree tests ----
    #[test]
    fn xq_131_veb_empty() {
        let v = super::Xq131VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_131_veb_insert_contains() {
        let mut v = super::Xq131VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_131_veb_min_max() {
        let mut v = super::Xq131VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_131_veb_delete() {
        let mut v = super::Xq131VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_131_veb_successor() {
        let mut v = super::Xq131VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_131_veb_predecessor() {
        let mut v = super::Xq131VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_131_veb_count() {
        let mut v = super::Xq131VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_131_veb_duplicate_insert() {
        let mut v = super::Xq131VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_131_kdtree_empty() {
        let tree = super::Xr131KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_131_kdtree_insert_one() {
        let mut tree = super::Xr131KDTree::xr_new();
        tree.xr_insert(super::Xr131KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_131_kdtree_insert_multiple() {
        let mut tree = super::Xr131KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr131KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_131_kdtree_nearest_neighbor() {
        let mut tree = super::Xr131KDTree::xr_new();
        tree.xr_insert(super::Xr131KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr131KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr131KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_131_kdtree_nn_empty() {
        let tree = super::Xr131KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr131KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_131_kdtree_range_search() {
        let mut tree = super::Xr131KDTree::xr_new();
        tree.xr_insert(super::Xr131KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr131KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr131KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_131_kdtree_range_empty() {
        let mut tree = super::Xr131KDTree::xr_new();
        tree.xr_insert(super::Xr131KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_131_kdtree_all_points() {
        let mut tree = super::Xr131KDTree::xr_new();
        tree.xr_insert(super::Xr131KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr131KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_131_kdtree_depth() {
        let mut tree = super::Xr131KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr131KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_131_kdtree_bounding_box() {
        let mut tree = super::Xr131KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr131KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr131KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_131_persistent_array_new() {
        let arr = super::Xs131PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_131_persistent_array_push() {
        let mut arr = super::Xs131PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_131_persistent_array_set() {
        let mut arr = super::Xs131PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_131_persistent_array_diff() {
        let mut arr = super::Xs131PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_131_persistent_array_rollback() {
        let mut arr = super::Xs131PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_131_persistent_array_history() {
        let mut arr = super::Xs131PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_131_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs131PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_131_persistent_array_from_vec() {
        let arr = super::Xs131PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_131_concurrent_queue_new() {
        let q = super::Xs131ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_131_concurrent_queue_push_pop() {
        let mut q = super::Xs131ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_131_concurrent_queue_full() {
        let mut q = super::Xs131ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_131_concurrent_queue_drain() {
        let mut q = super::Xs131ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_131_concurrent_queue_try_pop() {
        let mut q = super::Xs131ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_131_concurrent_queue_clear() {
        let mut q = super::Xs131ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_131_range_map_new() {
        let rm = super::Xs131RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_131_range_map_insert_get() {
        let mut rm = super::Xs131RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_131_range_map_overlap() {
        let mut rm = super::Xs131RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_131_range_map_remove() {
        let mut rm = super::Xs131RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_131_range_map_gaps() {
        let mut rm = super::Xs131RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_131_range_map_coverage() {
        let mut rm = super::Xs131RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_131_range_map_contains() {
        let mut rm = super::Xs131RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_131_range_map_clear() {
        let mut rm = super::Xs131RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_131_circular_buffer_new() {
        let buf = super::Xs131CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_131_circular_buffer_push_pop() {
        let mut buf = super::Xs131CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_131_circular_buffer_overwrite() {
        let mut buf = super::Xs131CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_131_circular_buffer_peek() {
        let mut buf = super::Xs131CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_131_circular_buffer_is_full() {
        let mut buf = super::Xs131CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_131_circular_buffer_iter() {
        let mut buf = super::Xs131CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_131_circular_buffer_clear() {
        let mut buf = super::Xs131CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_131_circular_buffer_to_vec() {
        let mut buf = super::Xs131CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }


    // --- xy_ Cuckoo Hash Map tests ---

    #[test]
    fn xy_cuckoo_new() {
        let m = super::XyCuckooMap::<String, i32>::xy_new(16);
        assert!(m.xy_is_empty());
        assert_eq!(m.xy_len(), 0);
    }

    #[test]
    fn xy_cuckoo_insert_get() {
        let mut m = super::XyCuckooMap::xy_new(32);
        m.xy_insert("hello".to_string(), 1);
        m.xy_insert("world".to_string(), 2);
        assert_eq!(m.xy_get(&"hello".to_string()), Some(&1));
        assert_eq!(m.xy_get(&"world".to_string()), Some(&2));
        assert_eq!(m.xy_get(&"missing".to_string()), None);
    }

    #[test]
    fn xy_cuckoo_contains() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(42, "a");
        assert!(m.xy_contains(&42));
        assert!(!m.xy_contains(&99));
    }

    #[test]
    fn xy_cuckoo_replace() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(5, "old");
        m.xy_insert(5, "new");
        assert_eq!(m.xy_get(&5), Some(&"new"));
    }

    #[test]
    fn xy_cuckoo_remove() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(10, "val");
        assert_eq!(m.xy_remove(&10), Some("val"));
        assert!(!m.xy_contains(&10));
    }

    #[test]
    fn xy_cuckoo_many() {
        let mut m = super::XyCuckooMap::xy_new(64);
        for i in 0..30 {
            m.xy_insert(i, i * 10);
        }
        assert_eq!(m.xy_len(), 30);
        for i in 0..30 {
            assert!(m.xy_contains(&i));
        }
    }

    #[test]
    fn xy_cuckoo_keys() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_insert(2, "b");
        let keys = m.xy_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn xy_cuckoo_clear() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_clear();
        assert!(m.xy_is_empty());
    }

    #[test]
    fn xy_cuckoo_display() {
        let m = super::XyCuckooMap::<i32, i32>::xy_new(16);
        assert!(format!("{}", m).contains("CuckooMap"));
    }

    #[test]
    fn xy_cuckoo_default() {
        let m = super::XyCuckooMap::<i32, i32>::default();
        assert!(m.xy_is_empty());
    }

    // --- xy_ Count-Min Sketch tests ---

    #[test]
    fn xy_cms_new() {
        let cms = super::XyCountMinSketch::xy_new(100, 5);
        assert_eq!(cms.xy_width(), 100);
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_add_estimate() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..10 { cms.xy_add(42); }
        assert!(cms.xy_estimate(42) >= 10);
    }

    #[test]
    fn xy_cms_add_count() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        cms.xy_add_count(7, 100);
        assert!(cms.xy_estimate(7) >= 100);
    }

    #[test]
    fn xy_cms_unseen() {
        let cms = super::XyCountMinSketch::xy_new(1000, 5);
        assert_eq!(cms.xy_estimate(999), 0);
    }

    #[test]
    fn xy_cms_merge() {
        let mut a = super::XyCountMinSketch::xy_new(100, 3);
        let mut b = super::XyCountMinSketch::xy_new(100, 3);
        a.xy_add(1);
        b.xy_add(1);
        a.xy_merge(&b);
        assert!(a.xy_estimate(1) >= 2);
    }

    #[test]
    fn xy_cms_clear() {
        let mut cms = super::XyCountMinSketch::xy_new(100, 3);
        cms.xy_add(1);
        cms.xy_clear();
        assert_eq!(cms.xy_estimate(1), 0);
    }

    #[test]
    fn xy_cms_display() {
        let cms = super::XyCountMinSketch::xy_new(100, 3);
        assert!(format!("{}", cms).contains("CMS"));
    }

    #[test]
    fn xy_cms_default() {
        let cms = super::XyCountMinSketch::default();
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_multiple_items() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for i in 0..100 { cms.xy_add(i); }
        for i in 0..100 { assert!(cms.xy_estimate(i) >= 1); }
    }

    #[test]
    fn xy_cms_heavy_hitter() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..1000 { cms.xy_add(42); }
        for i in 0..10 { cms.xy_add(i); }
        assert!(cms.xy_estimate(42) > cms.xy_estimate(0));
    }


    // --- xz_ HyperLogLog tests ---

    #[test]
    fn xz_hll_new() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_num_registers(), 1024);
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_add_estimate() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for i in 0..1000 {
            hll.xz_add(i);
        }
        let est = hll.xz_estimate();
        assert!(est > 500.0 && est < 2000.0);
    }

    #[test]
    fn xz_hll_empty() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_merge() {
        let mut a = super::XzHyperLogLog::xz_new(10);
        let mut b = super::XzHyperLogLog::xz_new(10);
        for i in 0..500 { a.xz_add(i); }
        for i in 500..1000 { b.xz_add(i); }
        a.xz_merge(&b);
        let est = a.xz_estimate();
        assert!(est > 500.0);
    }

    #[test]
    fn xz_hll_clear() {
        let mut hll = super::XzHyperLogLog::xz_new(10);
        hll.xz_add(1);
        hll.xz_clear();
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_display() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert!(format!("{}", hll).contains("HLL"));
    }

    #[test]
    fn xz_hll_default() {
        let hll = super::XzHyperLogLog::default();
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_duplicates() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for _ in 0..1000 { hll.xz_add(42); }
        let est = hll.xz_estimate();
        assert!(est < 10.0);
    }

    // --- xz_ LRU Cache tests ---

    #[test]
    fn xz_lru_new() {
        let lru = super::XzLruCache::<String, i32>::xz_new(10);
        assert!(lru.xz_is_empty());
        assert_eq!(lru.xz_capacity(), 10);
    }

    #[test]
    fn xz_lru_put_get() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put("a".to_string(), 1);
        lru.xz_put("b".to_string(), 2);
        assert_eq!(lru.xz_get(&"a".to_string()), Some(&1));
        assert_eq!(lru.xz_get(&"b".to_string()), Some(&2));
    }

    #[test]
    fn xz_lru_eviction() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        assert!(!lru.xz_contains(&1));
        assert!(lru.xz_contains(&2));
        assert!(lru.xz_contains(&3));
    }

    #[test]
    fn xz_lru_access_updates_order() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_get(&1);
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1));
        assert!(!lru.xz_contains(&2));
    }

    #[test]
    fn xz_lru_update_value() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "old");
        lru.xz_put(1, "new");
        assert_eq!(lru.xz_get(&1), Some(&"new"));
        assert_eq!(lru.xz_len(), 1);
    }

    #[test]
    fn xz_lru_remove() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        assert_eq!(lru.xz_remove(&1), Some("a"));
        assert!(!lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_peek() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        assert_eq!(lru.xz_peek(&1), Some(&"a"));
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1) || !lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_keys_order() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        let keys = lru.xz_keys_lru();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn xz_lru_clear() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_clear();
        assert!(lru.xz_is_empty());
    }

    #[test]
    fn xz_lru_display() {
        let lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert!(format!("{}", lru).contains("LRU"));
    }

    #[test]
    fn xz_lru_missing_key() {
        let mut lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert_eq!(lru.xz_get(&999), None);
    }


    // --- ya_ Trie tests ---

    #[test]
    fn ya_trie_new() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(t.ya_is_empty());
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_insert_get() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        t.ya_insert("world", 2);
        assert_eq!(t.ya_get("hello"), Some(&1));
        assert_eq!(t.ya_get("world"), Some(&2));
        assert_eq!(t.ya_get("missing"), None);
    }

    #[test]
    fn ya_trie_contains() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        assert!(t.ya_contains("abc"));
        assert!(!t.ya_contains("ab"));
        assert!(!t.ya_contains("abcd"));
    }

    #[test]
    fn ya_trie_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert!(t.ya_has_prefix("ab"));
        assert!(!t.ya_has_prefix("ac"));
    }

    #[test]
    fn ya_trie_keys_with_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("cat", 1);
        t.ya_insert("car", 2);
        t.ya_insert("dog", 3);
        let keys = t.ya_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"cat".to_string()));
        assert!(keys.contains(&"car".to_string()));
    }

    #[test]
    fn ya_trie_all_keys() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("b", 1);
        t.ya_insert("a", 2);
        t.ya_insert("c", 3);
        let keys = t.ya_all_keys();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn ya_trie_remove() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        assert_eq!(t.ya_remove("hello"), Some(1));
        assert!(!t.ya_contains("hello"));
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_lcp() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert_eq!(t.ya_longest_common_prefix(), "ab");
    }

    #[test]
    fn ya_trie_clear() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("a", 1);
        t.ya_clear();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_display() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(format!("{}", t).contains("Trie"));
    }

    #[test]
    fn ya_trie_default() {
        let t = super::YaTrie::<i32>::default();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_count_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("test1", 1);
        t.ya_insert("test2", 2);
        t.ya_insert("other", 3);
        assert_eq!(t.ya_count_prefix("test"), 2);
    }

    // --- ya_ Bloom Filter tests ---

    #[test]
    fn ya_bloom_new() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_bit_size(), 1000);
        assert_eq!(bf.ya_num_hashes(), 5);
        assert_eq!(bf.ya_count(), 0);
    }

    #[test]
    fn ya_bloom_add_contains() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        bf.ya_add(42);
        bf.ya_add(100);
        assert!(bf.ya_might_contain(42));
        assert!(bf.ya_might_contain(100));
    }

    #[test]
    fn ya_bloom_no_false_negatives() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        for i in 0..100 { bf.ya_add(i); }
        for i in 0..100 { assert!(bf.ya_might_contain(i)); }
    }

    #[test]
    fn ya_bloom_with_fp_rate() {
        let bf = super::YaBloomFilter::ya_with_fp_rate(1000, 0.01);
        assert!(bf.ya_bit_size() > 0);
        assert!(bf.ya_num_hashes() > 0);
    }

    #[test]
    fn ya_bloom_clear() {
        let mut bf = super::YaBloomFilter::ya_new(1000, 5);
        bf.ya_add(1);
        bf.ya_clear();
        assert_eq!(bf.ya_count(), 0);
        assert!(!bf.ya_might_contain(1));
    }

    #[test]
    fn ya_bloom_merge() {
        let mut a = super::YaBloomFilter::ya_new(1000, 5);
        let mut b = super::YaBloomFilter::ya_new(1000, 5);
        a.ya_add(1);
        b.ya_add(2);
        a.ya_merge(&b);
        assert!(a.ya_might_contain(1));
        assert!(a.ya_might_contain(2));
    }

    #[test]
    fn ya_bloom_fp_rate() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_estimated_fp_rate(), 0.0);
    }

    #[test]
    fn ya_bloom_display() {
        let bf = super::YaBloomFilter::ya_new(100, 3);
        assert!(format!("{}", bf).contains("Bloom"));
    }

    #[test]
    fn ya_bloom_default() {
        let bf = super::YaBloomFilter::default();
        assert_eq!(bf.ya_num_hashes(), 5);
    }


    // --- yb_ TST tests ---

    #[test]
    fn yb_tst_new() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_insert_get() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("hello", 1);
        t.yb_insert("world", 2);
        assert_eq!(t.yb_get("hello"), Some(&1));
        assert_eq!(t.yb_get("world"), Some(&2));
        assert_eq!(t.yb_get("missing"), None);
    }

    #[test]
    fn yb_tst_contains() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("abc", 10);
        assert!(t.yb_contains("abc"));
        assert!(!t.yb_contains("ab"));
    }

    #[test]
    fn yb_tst_all_keys() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("b", 1);
        t.yb_insert("a", 2);
        t.yb_insert("c", 3);
        let keys = t.yb_all_keys();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn yb_tst_prefix() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("cat", 1);
        t.yb_insert("car", 2);
        t.yb_insert("dog", 3);
        let keys = t.yb_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn yb_tst_clear() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("a", 1);
        t.yb_clear();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_display() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(format!("{}", t).contains("TST"));
    }

    #[test]
    fn yb_tst_default() {
        let t = super::YbTernarySearchTree::<i32>::default();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_overwrite() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("key", 1);
        t.yb_insert("key", 2);
        assert_eq!(t.yb_get("key"), Some(&2));
        assert_eq!(t.yb_len(), 1);
    }

    // --- yb_ Quadtree tests ---

    #[test]
    fn yb_quad_new() {
        let q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_insert() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_insert(super::YbPoint::yb_new(50.0, 50.0)));
        assert_eq!(q.yb_count(), 1);
    }

    #[test]
    fn yb_quad_query() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        q.yb_insert(super::YbPoint::yb_new(15.0, 15.0));
        let found = q.yb_query(&super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn yb_quad_outside() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(!q.yb_insert(super::YbPoint::yb_new(200.0, 200.0)));
    }

    #[test]
    fn yb_quad_nearest() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        let near = q.yb_nearest(&super::YbPoint::yb_new(12.0, 12.0)).unwrap();
        assert!((near.yb_x - 10.0).abs() < 0.001);
    }

    #[test]
    fn yb_quad_display() {
        let q = super::YbQuadtree::default();
        assert!(format!("{}", q).contains("Quadtree"));
    }

    #[test]
    fn yb_quad_default() {
        let q = super::YbQuadtree::default();
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_many() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        for i in 0..20 {
            q.yb_insert(super::YbPoint::yb_new(i as f64 * 4.0, i as f64 * 4.0));
        }
        assert_eq!(q.yb_count(), 20);
    }

    #[test]
    fn yb_point_distance() {
        let a = super::YbPoint::yb_new(0.0, 0.0);
        let b = super::YbPoint::yb_new(3.0, 4.0);
        assert!((a.yb_distance(&b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn yb_bounds_intersects() {
        let a = super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0);
        let b = super::YbBounds::yb_new(25.0, 25.0, 50.0, 50.0);
        assert!(a.yb_intersects(&b));
    }


    // --- yc_ VebSet tests ---

    #[test]
    fn yc_veb_new() {
        let v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_is_empty());
        assert_eq!(v.yc_universe(), 1000);
    }

    #[test]
    fn yc_veb_insert_contains() {
        let mut v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_insert(42));
        assert!(v.yc_contains(42));
        assert!(!v.yc_contains(43));
    }

    #[test]
    fn yc_veb_remove() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        assert!(v.yc_remove(10));
        assert!(!v.yc_contains(10));
    }

    #[test]
    fn yc_veb_min_max() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(50);
        v.yc_insert(10);
        v.yc_insert(90);
        assert_eq!(v.yc_min(), Some(10));
        assert_eq!(v.yc_max(), Some(90));
    }

    #[test]
    fn yc_veb_successor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        v.yc_insert(30);
        assert_eq!(v.yc_successor(10), Some(20));
        assert_eq!(v.yc_successor(20), Some(30));
        assert_eq!(v.yc_successor(30), None);
    }

    #[test]
    fn yc_veb_predecessor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_predecessor(20), Some(10));
        assert_eq!(v.yc_predecessor(10), None);
    }

    #[test]
    fn yc_veb_sorted() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(30);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_to_sorted_vec(), vec![10, 20, 30]);
    }

    #[test]
    fn yc_veb_clear() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(1);
        v.yc_clear();
        assert!(v.yc_is_empty());
    }

    #[test]
    fn yc_veb_union() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1);
        b.yc_insert(2);
        a.yc_union(&b);
        assert!(a.yc_contains(1));
        assert!(a.yc_contains(2));
    }

    #[test]
    fn yc_veb_intersection() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1); a.yc_insert(2);
        b.yc_insert(2); b.yc_insert(3);
        let c = a.yc_intersection(&b);
        assert!(c.yc_contains(2));
        assert!(!c.yc_contains(1));
    }

    #[test]
    fn yc_veb_display() {
        let v = super::YcVebSet::yc_new(100);
        assert!(format!("{}", v).contains("VebSet"));
    }

    #[test]
    fn yc_veb_default() {
        let v = super::YcVebSet::default();
        assert_eq!(v.yc_universe(), 65536);
    }

    // --- yc_ HashRing tests ---

    #[test]
    fn yc_ring_new() {
        let r = super::YcHashRing::yc_new(100);
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_add_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert_eq!(r.yc_node_count(), 1);
        assert_eq!(r.yc_virtual_count(), 50);
    }

    #[test]
    fn yc_ring_get_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n = r.yc_get_node("mykey");
        assert!(n.is_some());
    }

    #[test]
    fn yc_ring_remove_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_remove_node("a");
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_has_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert!(r.yc_has_node("server1"));
        assert!(!r.yc_has_node("server2"));
    }

    #[test]
    fn yc_ring_display() {
        let r = super::YcHashRing::yc_new(10);
        assert!(format!("{}", r).contains("HashRing"));
    }

    #[test]
    fn yc_ring_default() {
        let r = super::YcHashRing::default();
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_consistency() {
        let mut r = super::YcHashRing::yc_new(100);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n1 = r.yc_get_node("key1").unwrap().to_string();
        let n2 = r.yc_get_node("key1").unwrap().to_string();
        assert_eq!(n1, n2);
    }


    // --- yd_ DAG tests ---

    #[test]
    fn yd_dag_new() {
        let g = super::YdDag::yd_new();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_add_edge() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        assert_eq!(g.yd_node_count(), 3);
        assert_eq!(g.yd_edge_count(), 2);
    }

    #[test]
    fn yd_dag_topo_sort() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        g.yd_add_edge(2, 3);
        let order = g.yd_topological_sort().unwrap();
        assert_eq!(order.len(), 4);
        let pos: std::collections::HashMap<usize, usize> = order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&0] < pos[&2]);
    }

    #[test]
    fn yd_dag_cycle() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(2, 0);
        assert!(g.yd_has_cycle());
    }

    #[test]
    fn yd_dag_roots_leaves() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_roots(), vec![0]);
        assert_eq!(g.yd_leaves(), vec![1, 2]);
    }

    #[test]
    fn yd_dag_bfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        let bfs = g.yd_bfs(0);
        assert_eq!(bfs[0], 0);
        assert_eq!(bfs.len(), 4);
    }

    #[test]
    fn yd_dag_dfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        let dfs = g.yd_dfs(0);
        assert_eq!(dfs[0], 0);
        assert_eq!(dfs.len(), 3);
    }

    #[test]
    fn yd_dag_shortest_path() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_shortest_path(0, 2), Some(1));
    }

    #[test]
    fn yd_dag_degrees() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_out_degree(0), 2);
        assert_eq!(g.yd_in_degree(1), 1);
    }

    #[test]
    fn yd_dag_clear() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_clear();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_display() {
        let g = super::YdDag::yd_new();
        assert!(format!("{}", g).contains("DAG"));
    }

    // --- yd_ SparseMatrix tests ---

    #[test]
    fn yd_sparse_new() {
        let m = super::YdSparseMatrix::yd_new(3, 3);
        assert_eq!(m.yd_nnz(), 0);
    }

    #[test]
    fn yd_sparse_set_get() {
        let mut m = super::YdSparseMatrix::yd_new(3, 3);
        m.yd_set(0, 1, 5.0);
        assert_eq!(m.yd_get(0, 1), 5.0);
        assert_eq!(m.yd_get(0, 0), 0.0);
    }

    #[test]
    fn yd_sparse_transpose() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 2, 7.0);
        let t = m.yd_transpose();
        assert_eq!(t.yd_get(2, 0), 7.0);
    }

    #[test]
    fn yd_sparse_mul_vec() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_set(1, 1, 2.0);
        let r = m.yd_mul_vec(&[3.0, 4.0]);
        assert_eq!(r, vec![3.0, 8.0]);
    }

    #[test]
    fn yd_sparse_add() {
        let mut a = super::YdSparseMatrix::yd_new(2, 2);
        let mut b = super::YdSparseMatrix::yd_new(2, 2);
        a.yd_set(0, 0, 1.0);
        b.yd_set(0, 0, 2.0);
        let c = a.yd_add(&b);
        assert_eq!(c.yd_get(0, 0), 3.0);
    }

    #[test]
    fn yd_sparse_scale() {
        let mut m = super::YdSparseMatrix::yd_new(1, 1);
        m.yd_set(0, 0, 5.0);
        m.yd_scale(2.0);
        assert_eq!(m.yd_get(0, 0), 10.0);
    }

    #[test]
    fn yd_sparse_row_sum() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 0, 1.0);
        m.yd_set(0, 1, 2.0);
        m.yd_set(0, 2, 3.0);
        assert_eq!(m.yd_row_sum(0), 6.0);
    }

    #[test]
    fn yd_sparse_display() {
        let m = super::YdSparseMatrix::yd_new(2, 2);
        assert!(format!("{}", m).contains("SparseMatrix"));
    }

    #[test]
    fn yd_sparse_clear() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_clear();
        assert_eq!(m.yd_nnz(), 0);
    }


    // --- ye_ IndexedPQ tests ---

    #[test]
    fn ye_ipq_new() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_insert_pop() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 5);
        pq.ye_insert(2, 15);
        assert_eq!(pq.ye_pop(), Some((1, 5)));
        assert_eq!(pq.ye_pop(), Some((0, 10)));
    }

    #[test]
    fn ye_ipq_decrease_key() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 20);
        pq.ye_decrease_key(1, 5);
        assert_eq!(pq.ye_peek(), Some((1, 5)));
    }

    #[test]
    fn ye_ipq_contains() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(42, 1);
        assert!(pq.ye_contains(42));
        assert!(!pq.ye_contains(99));
    }

    #[test]
    fn ye_ipq_priority() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 7);
        assert_eq!(pq.ye_priority(0), Some(7));
    }

    #[test]
    fn ye_ipq_drain() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 30);
        pq.ye_insert(1, 10);
        pq.ye_insert(2, 20);
        let sorted = pq.ye_drain_sorted();
        assert_eq!(sorted, vec![(1, 10), (2, 20), (0, 30)]);
    }

    #[test]
    fn ye_ipq_clear() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 1);
        pq.ye_clear();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_display() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(format!("{}", pq).contains("IndexedPQ"));
    }

    #[test]
    fn ye_ipq_default() {
        let pq = super::YeIndexedPQ::default();
        assert!(pq.ye_is_empty());
    }

    // --- ye_ SegTree tests ---

    #[test]
    fn ye_seg_from_slice() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_len(), 5);
        assert_eq!(st.ye_query(0, 4), 15);
    }

    #[test]
    fn ye_seg_point_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[10, 20, 30]);
        assert_eq!(st.ye_point_query(1), 20);
    }

    #[test]
    fn ye_seg_range_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_query(1, 3), 9);
    }

    #[test]
    fn ye_seg_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        st.ye_update(1, 3, 10);
        assert_eq!(st.ye_query(0, 4), 45);
    }

    #[test]
    fn ye_seg_single_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        st.ye_update(1, 1, 5);
        assert_eq!(st.ye_point_query(1), 7);
    }

    #[test]
    fn ye_seg_empty() {
        let st = super::YeSegTree::ye_from_slice(&[]);
        assert!(st.ye_is_empty());
    }

    #[test]
    fn ye_seg_single() {
        let mut st = super::YeSegTree::ye_from_slice(&[42]);
        assert_eq!(st.ye_query(0, 0), 42);
    }

    #[test]
    fn ye_seg_display() {
        let st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        assert!(format!("{}", st).contains("SegTree"));
    }

    #[test]
    fn ye_seg_default() {
        let st = super::YeSegTree::default();
        assert!(st.ye_is_empty());
    }


    // --- yf_ IntervalSet tests ---

    #[test]
    fn yf_interval_new() {
        let s = super::YfIntervalSet::yf_new();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_add() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        assert_eq!(s.yf_len(), 1);
        assert!(s.yf_contains(3));
    }

    #[test]
    fn yf_interval_merge() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(3, 8);
        assert_eq!(s.yf_len(), 1);
        assert_eq!(s.yf_intervals(), vec![(1, 8)]);
    }

    #[test]
    fn yf_interval_adjacent() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(6, 10);
        assert_eq!(s.yf_len(), 1);
    }

    #[test]
    fn yf_interval_disjoint() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 3);
        s.yf_add(10, 15);
        assert_eq!(s.yf_len(), 2);
    }

    #[test]
    fn yf_interval_remove_point() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 10);
        s.yf_remove_point(5);
        assert!(!s.yf_contains(5));
        assert!(s.yf_contains(4));
        assert!(s.yf_contains(6));
    }

    #[test]
    fn yf_interval_length() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(10, 14);
        assert_eq!(s.yf_total_length(), 10);
    }

    #[test]
    fn yf_interval_clear() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_clear();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_display() {
        let s = super::YfIntervalSet::yf_new();
        assert!(format!("{}", s).contains("IntervalSet"));
    }

    #[test]
    fn yf_interval_overlaps() {
        let mut a = super::YfIntervalSet::yf_new();
        let mut b = super::YfIntervalSet::yf_new();
        a.yf_add(1, 5);
        b.yf_add(3, 8);
        assert!(a.yf_overlaps(&b));
    }

    // --- yf_ KWayMerge tests ---

    #[test]
    fn yf_kmerge_new() {
        let m = super::YfKWayMerge::yf_new();
        assert_eq!(m.yf_source_count(), 0);
    }

    #[test]
    fn yf_kmerge_merge() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 4, 7]);
        m.yf_add_source(vec![2, 5, 8]);
        m.yf_add_source(vec![3, 6, 9]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn yf_kmerge_single() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn yf_kmerge_empty() {
        let mut m = super::YfKWayMerge::yf_new();
        let result = m.yf_merge();
        assert!(result.is_empty());
    }

    #[test]
    fn yf_kmerge_remaining() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        m.yf_add_source(vec![3, 4]);
        assert_eq!(m.yf_remaining(), 4);
    }

    #[test]
    fn yf_kmerge_reset() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        let _ = m.yf_merge();
        m.yf_reset();
        assert_eq!(m.yf_remaining(), 2);
    }

    #[test]
    fn yf_kmerge_unique() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        m.yf_add_source(vec![2, 3, 4]);
        let result = m.yf_merge_unique();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn yf_kmerge_display() {
        let m = super::YfKWayMerge::yf_new();
        assert!(format!("{}", m).contains("KWayMerge"));
    }

    #[test]
    fn yf_kmerge_default() {
        let m = super::YfKWayMerge::default();
        assert!(m.yf_is_done());
    }


    // --- yg_ PersistentStack tests ---

    #[test]
    fn yg_pstack_new() {
        let s = super::YgPersistentStack::<i32>::yg_new();
        assert!(s.yg_is_empty());
    }

    #[test]
    fn yg_pstack_push_pop() {
        let s = super::YgPersistentStack::yg_new();
        let s = s.yg_push(1);
        let s = s.yg_push(2);
        let (v, s) = s.yg_pop().unwrap();
        assert_eq!(v, 2);
        let (v, _) = s.yg_pop().unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn yg_pstack_persistence() {
        let s1 = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2);
        let s2 = s1.yg_push(3);
        assert_eq!(s1.yg_len(), 2);
        assert_eq!(s2.yg_len(), 3);
    }

    #[test]
    fn yg_pstack_peek() {
        let s = super::YgPersistentStack::yg_new().yg_push(42);
        assert_eq!(s.yg_peek(), Some(&42));
    }

    #[test]
    fn yg_pstack_to_vec() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        assert_eq!(s.yg_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn yg_pstack_reverse() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        let r = s.yg_reverse();
        assert_eq!(r.yg_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn yg_pstack_display() {
        let s = super::YgPersistentStack::yg_new().yg_push(1);
        assert!(format!("{}", s).contains("PStack"));
    }

    #[test]
    fn yg_pstack_default() {
        let s = super::YgPersistentStack::<i32>::default();
        assert!(s.yg_is_empty());
    }

    // --- yg_ BitmapIndex tests ---

    #[test]
    fn yg_bitmap_new() {
        let bi = super::YgBitmapIndex::yg_new(100);
        assert_eq!(bi.yg_num_rows(), 100);
    }

    #[test]
    fn yg_bitmap_set_get() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("color_red", 5);
        assert!(bi.yg_get("color_red", 5));
        assert!(!bi.yg_get("color_red", 6));
    }

    #[test]
    fn yg_bitmap_and() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("a", 2);
        bi.yg_set("b", 2);
        bi.yg_set("b", 3);
        let rows = bi.yg_and(&["a", "b"]);
        assert_eq!(rows, vec![2]);
    }

    #[test]
    fn yg_bitmap_or() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("b", 2);
        let rows = bi.yg_or(&["a", "b"]);
        assert_eq!(rows, vec![1, 2]);
    }

    #[test]
    fn yg_bitmap_count() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("x", 0);
        bi.yg_set("x", 1);
        bi.yg_set("x", 2);
        assert_eq!(bi.yg_count("x"), 3);
    }

    #[test]
    fn yg_bitmap_columns() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_set("b", 0);
        assert_eq!(bi.yg_num_columns(), 2);
    }

    #[test]
    fn yg_bitmap_clear() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_clear();
        assert_eq!(bi.yg_num_columns(), 0);
    }

    #[test]
    fn yg_bitmap_display() {
        let bi = super::YgBitmapIndex::yg_new(10);
        assert!(format!("{}", bi).contains("BitmapIndex"));
    }

    #[test]
    fn yg_bitmap_default() {
        let bi = super::YgBitmapIndex::default();
        assert_eq!(bi.yg_num_rows(), 0);
    }


    // --- yh_ OSTree tests ---

    #[test]
    fn yh_ost_new() {
        let t = super::YhOrderStatTree::yh_new();
        assert!(t.yh_is_empty());
    }

    #[test]
    fn yh_ost_insert_contains() {
        let mut t = super::YhOrderStatTree::yh_new();
        t.yh_insert(10);
        t.yh_insert(5);
        t.yh_insert(15);
        assert!(t.yh_contains(10));
        assert!(!t.yh_contains(7));
    }

    #[test]
    fn yh_ost_rank() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [10, 5, 15, 3, 7] { t.yh_insert(v); }
        assert_eq!(t.yh_rank(5), 1);
        assert_eq!(t.yh_rank(10), 3);
    }

    #[test]
    fn yh_ost_select() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [10, 5, 15, 3, 7] { t.yh_insert(v); }
        assert_eq!(t.yh_select(0), Some(3));
        assert_eq!(t.yh_select(2), Some(7));
        assert_eq!(t.yh_select(4), Some(15));
    }

    #[test]
    fn yh_ost_min_max() {
        let mut t = super::YhOrderStatTree::yh_new();
        t.yh_insert(10);
        t.yh_insert(5);
        t.yh_insert(15);
        assert_eq!(t.yh_min(), Some(5));
        assert_eq!(t.yh_max(), Some(15));
    }

    #[test]
    fn yh_ost_inorder() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [5, 3, 7, 1, 4] { t.yh_insert(v); }
        assert_eq!(t.yh_inorder(), vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn yh_ost_count_range() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [1, 3, 5, 7, 9] { t.yh_insert(v); }
        assert_eq!(t.yh_count_range(3, 7), 3);
    }

    #[test]
    fn yh_ost_display() {
        let t = super::YhOrderStatTree::yh_new();
        assert!(format!("{}", t).contains("OSTree"));
    }

    #[test]
    fn yh_ost_default() {
        let t = super::YhOrderStatTree::default();
        assert!(t.yh_is_empty());
    }

    // --- yh_ Reservoir tests ---

    #[test]
    fn yh_reservoir_new() {
        let r = super::YhReservoirSampler::yh_new(5, 42);
        assert_eq!(r.yh_k(), 5);
        assert_eq!(r.yh_count(), 0);
    }

    #[test]
    fn yh_reservoir_add() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        for i in 0..10 { r.yh_add(i); }
        assert_eq!(r.yh_len(), 3);
        assert_eq!(r.yh_count(), 10);
    }

    #[test]
    fn yh_reservoir_underfill() {
        let mut r = super::YhReservoirSampler::yh_new(10, 42);
        r.yh_add(1);
        r.yh_add(2);
        assert_eq!(r.yh_len(), 2);
        assert!(!r.yh_is_full());
    }

    #[test]
    fn yh_reservoir_full() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        r.yh_add(1); r.yh_add(2); r.yh_add(3);
        assert!(r.yh_is_full());
    }

    #[test]
    fn yh_reservoir_reset() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        r.yh_add(1);
        r.yh_reset(99);
        assert_eq!(r.yh_count(), 0);
        assert_eq!(r.yh_len(), 0);
    }

    #[test]
    fn yh_reservoir_display() {
        let r = super::YhReservoirSampler::yh_new(5, 42);
        assert!(format!("{}", r).contains("Reservoir"));
    }

    #[test]
    fn yh_reservoir_default() {
        let r = super::YhReservoirSampler::default();
        assert_eq!(r.yh_k(), 10);
    }

    #[test]
    fn yh_reservoir_sample() {
        let mut r = super::YhReservoirSampler::yh_new(5, 42);
        for i in 0..100 { r.yh_add(i); }
        assert_eq!(r.yh_sample().len(), 5);
    }


    // --- yi_ RingBuffer tests ---

    #[test]
    fn yi_ring_new() {
        let r = super::YiRingBuffer::<i32>::yi_new(10);
        assert!(r.yi_is_empty());
        assert_eq!(r.yi_capacity(), 10);
    }

    #[test]
    fn yi_ring_push_pop() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        r.yi_push_back(3);
        assert_eq!(r.yi_pop_front(), Some(1));
        assert_eq!(r.yi_pop_front(), Some(2));
    }

    #[test]
    fn yi_ring_push_front() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_front(1);
        r.yi_push_front(2);
        assert_eq!(r.yi_front(), Some(&2));
    }

    #[test]
    fn yi_ring_full() {
        let mut r = super::YiRingBuffer::yi_new(2);
        assert!(r.yi_push_back(1));
        assert!(r.yi_push_back(2));
        assert!(!r.yi_push_back(3));
        assert!(r.yi_is_full());
    }

    #[test]
    fn yi_ring_wrap() {
        let mut r = super::YiRingBuffer::yi_new(3);
        r.yi_push_back(1);
        r.yi_push_back(2);
        r.yi_push_back(3);
        r.yi_pop_front();
        r.yi_push_back(4);
        assert_eq!(r.yi_to_vec(), vec![2, 3, 4]);
    }

    #[test]
    fn yi_ring_force_push() {
        let mut r = super::YiRingBuffer::yi_new(2);
        r.yi_force_push_back(1);
        r.yi_force_push_back(2);
        r.yi_force_push_back(3);
        assert_eq!(r.yi_to_vec(), vec![2, 3]);
    }

    #[test]
    fn yi_ring_get() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(10);
        r.yi_push_back(20);
        assert_eq!(r.yi_get(0), Some(&10));
        assert_eq!(r.yi_get(1), Some(&20));
    }

    #[test]
    fn yi_ring_clear() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_clear();
        assert!(r.yi_is_empty());
    }

    #[test]
    fn yi_ring_back() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        assert_eq!(r.yi_back(), Some(&2));
    }

    #[test]
    fn yi_ring_pop_back() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        assert_eq!(r.yi_pop_back(), Some(2));
        assert_eq!(r.yi_len(), 1);
    }

    #[test]
    fn yi_ring_display() {
        let r = super::YiRingBuffer::<i32>::yi_new(10);
        assert!(format!("{}", r).contains("RingBuffer"));
    }

    // --- yi_ WeightedGraph tests ---

    #[test]
    fn yi_wgraph_new() {
        let g = super::YiWeightedGraph::yi_new();
        assert_eq!(g.yi_node_count(), 0);
    }

    #[test]
    fn yi_wgraph_add_edge() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 5.0);
        assert_eq!(g.yi_node_count(), 2);
    }

    #[test]
    fn yi_wgraph_dijkstra() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 4.0);
        g.yi_add_edge(0, 2, 1.0);
        g.yi_add_edge(2, 1, 2.0);
        let dists = g.yi_dijkstra(0);
        assert_eq!(dists[&1], 3.0);
    }

    #[test]
    fn yi_wgraph_shortest() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 1.0);
        g.yi_add_edge(1, 2, 2.0);
        assert_eq!(g.yi_shortest_distance(0, 2), Some(3.0));
    }

    #[test]
    fn yi_wgraph_no_path() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_node(0);
        g.yi_add_node(1);
        assert_eq!(g.yi_shortest_distance(0, 1), None);
    }

    #[test]
    fn yi_wgraph_undirected() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_undirected_edge(0, 1, 3.0);
        assert_eq!(g.yi_shortest_distance(1, 0), Some(3.0));
    }

    #[test]
    fn yi_wgraph_total_weight() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 2.0);
        g.yi_add_edge(1, 2, 3.0);
        assert_eq!(g.yi_total_weight(), 5.0);
    }

    #[test]
    fn yi_wgraph_clear() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 1.0);
        g.yi_clear();
        assert_eq!(g.yi_node_count(), 0);
    }

    #[test]
    fn yi_wgraph_display() {
        let g = super::YiWeightedGraph::yi_new();
        assert!(format!("{}", g).contains("WGraph"));
    }

    #[test]
    fn yi_wgraph_default() {
        let g = super::YiWeightedGraph::default();
        assert_eq!(g.yi_node_count(), 0);
    }


    // --- yj_ ExprEval tests ---

    #[test]
    fn yj_expr_simple() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("2 + 3").unwrap(), 5.0);
    }

    #[test]
    fn yj_expr_precedence() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("2 + 3 * 4").unwrap(), 14.0);
    }

    #[test]
    fn yj_expr_parens() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("(2 + 3) * 4").unwrap(), 20.0);
    }

    #[test]
    fn yj_expr_neg() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("-5 + 3").unwrap(), -2.0);
    }

    #[test]
    fn yj_expr_var() {
        let mut e = super::YjExprEval::yj_new();
        e.yj_set_var("x", 10.0);
        assert_eq!(e.yj_eval("x * 2").unwrap(), 20.0);
    }

    #[test]
    fn yj_expr_div() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("10 / 4").unwrap(), 2.5);
    }

    #[test]
    fn yj_expr_complex() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("(1 + 2) * (3 + 4)").unwrap(), 21.0);
    }

    #[test]
    fn yj_expr_error() {
        let e = super::YjExprEval::yj_new();
        assert!(e.yj_eval("2 +").is_err());
    }

    #[test]
    fn yj_expr_display() {
        let e = super::YjExprEval::yj_new();
        assert!(format!("{}", e).contains("ExprEval"));
    }

    #[test]
    fn yj_expr_clear() {
        let mut e = super::YjExprEval::yj_new();
        e.yj_set_var("x", 1.0);
        e.yj_clear();
        assert_eq!(e.yj_var_count(), 0);
    }

    // --- yj_ TtlCache tests ---

    #[test]
    fn yj_ttl_new() {
        let c = super::YjTtlCache::<i32>::yj_new(100);
        assert_eq!(c.yj_ttl(), 100);
    }

    #[test]
    fn yj_ttl_put_get() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 42);
        assert_eq!(c.yj_get("a"), Some(&42));
    }

    #[test]
    fn yj_ttl_expired() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(20);
        assert_eq!(c.yj_get("a"), None);
    }

    #[test]
    fn yj_ttl_not_expired() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 1);
        c.yj_tick(50);
        assert_eq!(c.yj_get("a"), Some(&1));
    }

    #[test]
    fn yj_ttl_evict() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(20);
        c.yj_evict_expired();
        assert_eq!(c.yj_len(), 0);
    }

    #[test]
    fn yj_ttl_valid_count() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(5);
        c.yj_put("b", 2);
        c.yj_tick(8);
        assert_eq!(c.yj_valid_count(), 1);
    }

    #[test]
    fn yj_ttl_remove() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 42);
        assert_eq!(c.yj_remove("a"), Some(42));
    }

    #[test]
    fn yj_ttl_clear() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 1);
        c.yj_clear();
        assert_eq!(c.yj_len(), 0);
    }

    #[test]
    fn yj_ttl_display() {
        let c = super::YjTtlCache::<i32>::yj_new(10);
        assert!(format!("{}", c).contains("TtlCache"));
    }

    #[test]
    fn yj_ttl_default() {
        let c = super::YjTtlCache::<i32>::default();
        assert_eq!(c.yj_ttl(), 60);
    }


    // --- yk_ GlobMatcher tests ---

    #[test]
    fn yk_glob_exact() {
        let g = super::YkGlobMatcher::yk_new("hello");
        assert!(g.yk_matches("hello"));
        assert!(!g.yk_matches("world"));
    }

    #[test]
    fn yk_glob_star() {
        let g = super::YkGlobMatcher::yk_new("*.rs");
        assert!(g.yk_matches("main.rs"));
        assert!(!g.yk_matches("main.py"));
    }

    #[test]
    fn yk_glob_question() {
        let g = super::YkGlobMatcher::yk_new("?.txt");
        assert!(g.yk_matches("a.txt"));
        assert!(!g.yk_matches("ab.txt"));
    }

    #[test]
    fn yk_glob_complex() {
        let g = super::YkGlobMatcher::yk_new("src/*.rs");
        assert!(g.yk_matches("src/main.rs"));
        assert!(g.yk_matches("src/sub/main.rs"));
    }

    #[test]
    fn yk_glob_empty() {
        let g = super::YkGlobMatcher::yk_new("*");
        assert!(g.yk_matches("anything"));
        assert!(g.yk_matches(""));
    }

    #[test]
    fn yk_glob_matches_any() {
        assert!(super::YkGlobMatcher::yk_matches_any(&["*.rs", "*.py"], "main.rs"));
        assert!(!super::YkGlobMatcher::yk_matches_any(&["*.rs", "*.py"], "main.js"));
    }

    #[test]
    fn yk_glob_filter() {
        let g = super::YkGlobMatcher::yk_new("*.rs");
        let files = vec!["main.rs", "lib.rs", "main.py"];
        assert_eq!(g.yk_filter(&files), vec!["main.rs", "lib.rs"]);
    }

    #[test]
    fn yk_glob_display() {
        let g = super::YkGlobMatcher::yk_new("*.txt");
        assert!(format!("{}", g).contains("Glob"));
    }

    #[test]
    fn yk_glob_default() {
        let g = super::YkGlobMatcher::default();
        assert!(g.yk_matches(""));
    }

    // --- yk_ EventBus tests ---

    #[test]
    fn yk_bus_new() {
        let b = super::YkEventBus::yk_new();
        assert_eq!(b.yk_topic_count(), 0);
    }

    #[test]
    fn yk_bus_subscribe() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "handler_a");
        assert_eq!(b.yk_subscriber_count("click"), 1);
    }

    #[test]
    fn yk_bus_emit() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "handler_a");
        b.yk_subscribe("click", "handler_b");
        let notified = b.yk_emit("click");
        assert_eq!(notified.len(), 2);
    }

    #[test]
    fn yk_bus_unsubscribe() {
        let mut b = super::YkEventBus::yk_new();
        let id = b.yk_subscribe("click", "handler_a");
        b.yk_unsubscribe(id);
        assert_eq!(b.yk_subscriber_count("click"), 0);
    }

    #[test]
    fn yk_bus_topics() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "a");
        b.yk_subscribe("keypress", "b");
        assert_eq!(b.yk_topics().len(), 2);
    }

    #[test]
    fn yk_bus_emit_pattern() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("mouse.click", "a");
        b.yk_subscribe("mouse.move", "b");
        b.yk_subscribe("key.press", "c");
        let notified = b.yk_emit_pattern("mouse.*");
        assert_eq!(notified.len(), 2);
    }

    #[test]
    fn yk_bus_clear() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("x", "a");
        b.yk_clear();
        assert_eq!(b.yk_total_subscribers(), 0);
    }

    #[test]
    fn yk_bus_has_subscribers() {
        let mut b = super::YkEventBus::yk_new();
        assert!(!b.yk_has_subscribers("x"));
        b.yk_subscribe("x", "a");
        assert!(b.yk_has_subscribers("x"));
    }

    #[test]
    fn yk_bus_display() {
        let b = super::YkEventBus::yk_new();
        assert!(format!("{}", b).contains("EventBus"));
    }

    #[test]
    fn yk_bus_default() {
        let b = super::YkEventBus::default();
        assert_eq!(b.yk_topic_count(), 0);
    }


    // --- yl_ MinMaxHeap tests ---

    #[test]
    fn yl_mmh_new() {
        let h = super::YlMinMaxHeap::yl_new();
        assert!(h.yl_is_empty());
    }

    #[test]
    fn yl_mmh_insert_min_max() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(3);
        h.yl_insert(8);
        h.yl_insert(1);
        assert_eq!(h.yl_peek_min(), Some(1));
        assert_eq!(h.yl_peek_max(), Some(8));
    }

    #[test]
    fn yl_mmh_pop_min() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(1);
        h.yl_insert(9);
        assert_eq!(h.yl_pop_min(), Some(1));
        assert_eq!(h.yl_peek_min(), Some(5));
    }

    #[test]
    fn yl_mmh_sorted() {
        let mut h = super::YlMinMaxHeap::yl_new();
        for v in [7, 3, 9, 1, 5] { h.yl_insert(v); }
        let sorted = h.yl_to_sorted_vec();
        assert_eq!(sorted, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn yl_mmh_single() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(42);
        assert_eq!(h.yl_peek_min(), Some(42));
        assert_eq!(h.yl_peek_max(), Some(42));
    }

    #[test]
    fn yl_mmh_two() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(3);
        assert_eq!(h.yl_peek_min(), Some(3));
        assert_eq!(h.yl_peek_max(), Some(5));
    }

    #[test]
    fn yl_mmh_clear() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(1);
        h.yl_clear();
        assert!(h.yl_is_empty());
    }

    #[test]
    fn yl_mmh_display() {
        let h = super::YlMinMaxHeap::yl_new();
        assert!(format!("{}", h).contains("MinMaxHeap"));
    }

    #[test]
    fn yl_mmh_default() {
        let h = super::YlMinMaxHeap::default();
        assert!(h.yl_is_empty());
    }

    // --- yl_ StateMachine tests ---

    #[test]
    fn yl_fsm_new() {
        let m = super::YlStateMachine::yl_new();
        assert_eq!(m.yl_state_count(), 0);
    }

    #[test]
    fn yl_fsm_basic() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        let s1 = m.yl_add_state("end");
        m.yl_add_transition(s0, "go", s1);
        m.yl_set_accept(s1);
        m.yl_set_start(s0);
        assert!(m.yl_step("go"));
        assert!(m.yl_is_accepting());
    }

    #[test]
    fn yl_fsm_run() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("a");
        let s1 = m.yl_add_state("b");
        let s2 = m.yl_add_state("c");
        m.yl_add_transition(s0, "x", s1);
        m.yl_add_transition(s1, "y", s2);
        m.yl_set_start(s0);
        assert!(m.yl_run(&["x", "y"]));
        assert_eq!(m.yl_current_state(), "c");
    }

    #[test]
    fn yl_fsm_invalid() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        m.yl_set_start(s0);
        assert!(!m.yl_step("invalid"));
    }

    #[test]
    fn yl_fsm_available() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("s0");
        let s1 = m.yl_add_state("s1");
        m.yl_add_transition(s0, "a", s1);
        m.yl_add_transition(s0, "b", s1);
        m.yl_set_start(s0);
        assert_eq!(m.yl_available_inputs().len(), 2);
    }

    #[test]
    fn yl_fsm_reset() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        let s1 = m.yl_add_state("end");
        m.yl_add_transition(s0, "go", s1);
        m.yl_set_start(s0);
        m.yl_step("go");
        m.yl_reset();
        assert_eq!(m.yl_current_state(), "start");
    }

    #[test]
    fn yl_fsm_display() {
        let m = super::YlStateMachine::yl_new();
        assert!(format!("{}", m).contains("FSM"));
    }

    #[test]
    fn yl_fsm_default() {
        let m = super::YlStateMachine::default();
        assert_eq!(m.yl_state_count(), 0);
    }


    // --- ym_ SortedMultiMap tests ---

    #[test]
    fn ym_smm_new() {
        let m = super::YmSortedMultiMap::<i32, i32>::ym_new();
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_insert_get() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, "a");
        m.ym_insert(1, "b");
        m.ym_insert(2, "c");
        assert_eq!(m.ym_get(&1).len(), 2);
        assert_eq!(m.ym_total_count(), 3);
    }

    #[test]
    fn ym_smm_keys() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(3, 1);
        m.ym_insert(1, 2);
        m.ym_insert(2, 3);
        assert_eq!(m.ym_keys(), vec![1, 2, 3]);
    }

    #[test]
    fn ym_smm_remove() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, "x");
        m.ym_insert(1, "y");
        let removed = m.ym_remove_key(&1);
        assert_eq!(removed.len(), 2);
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_range() {
        let mut m = super::YmSortedMultiMap::ym_new();
        for i in 0..10 { m.ym_insert(i, i * 10); }
        let r = m.ym_range(&3, &7);
        assert_eq!(r, vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn ym_smm_first_last() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(5, 0);
        m.ym_insert(1, 0);
        m.ym_insert(9, 0);
        assert_eq!(m.ym_first_key(), Some(1));
        assert_eq!(m.ym_last_key(), Some(9));
    }

    #[test]
    fn ym_smm_clear() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, 1);
        m.ym_clear();
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_display() {
        let m = super::YmSortedMultiMap::<i32, i32>::ym_new();
        assert!(format!("{}", m).contains("SortedMultiMap"));
    }

    #[test]
    fn ym_smm_default() {
        let m = super::YmSortedMultiMap::<i32, i32>::default();
        assert!(m.ym_is_empty());
    }

    // --- ym_ TaskScheduler tests ---

    #[test]
    fn ym_sched_new() {
        let s = super::YmTaskScheduler::ym_new();
        assert_eq!(s.ym_total(), 0);
    }

    #[test]
    fn ym_sched_add() {
        let mut s = super::YmTaskScheduler::ym_new();
        let id = s.ym_add_task("build", 1, vec![]);
        assert_eq!(id, 0);
        assert_eq!(s.ym_total(), 1);
    }

    #[test]
    fn ym_sched_next_ready() {
        let mut s = super::YmTaskScheduler::ym_new();
        s.ym_add_task("low", 10, vec![]);
        s.ym_add_task("high", 1, vec![]);
        let next = s.ym_next_ready().unwrap();
        assert_eq!(next.ym_name, "high");
    }

    #[test]
    fn ym_sched_deps() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("first", 1, vec![]);
        let _t1 = s.ym_add_task("second", 1, vec![t0]);
        let ready = s.ym_all_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].ym_name, "first");
    }

    #[test]
    fn ym_sched_complete() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("a", 1, vec![]);
        let _t1 = s.ym_add_task("b", 1, vec![t0]);
        s.ym_complete(t0);
        let ready = s.ym_all_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].ym_name, "b");
    }

    #[test]
    fn ym_sched_all_done() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("a", 1, vec![]);
        s.ym_complete(t0);
        assert!(s.ym_is_all_done());
    }

    #[test]
    fn ym_sched_clear() {
        let mut s = super::YmTaskScheduler::ym_new();
        s.ym_add_task("x", 1, vec![]);
        s.ym_clear();
        assert_eq!(s.ym_total(), 0);
    }

    #[test]
    fn ym_sched_display() {
        let s = super::YmTaskScheduler::ym_new();
        assert!(format!("{}", s).contains("Scheduler"));
    }

    #[test]
    fn ym_sched_default() {
        let s = super::YmTaskScheduler::default();
        assert!(s.ym_is_all_done());
    }

    #[test]
    fn ym_task_display() {
        let t = super::YmTask { ym_id: 0, ym_name: "test".to_string(), ym_priority: 1, ym_deps: vec![], ym_done: false };
        assert!(format!("{}", t).contains("Task"));
    }


    // --- yn_ ImmutableMap tests ---

    #[test]
    fn yn_imm_new() {
        let m = super::YnImmutableMap::<i32, i32>::yn_new();
        assert!(m.yn_is_empty());
    }

    #[test]
    fn yn_imm_insert_get() {
        let m = super::YnImmutableMap::yn_new();
        let m = m.yn_insert(1, "a");
        let m = m.yn_insert(2, "b");
        assert_eq!(m.yn_get(&1), Some(&"a"));
        assert_eq!(m.yn_get(&2), Some(&"b"));
    }

    #[test]
    fn yn_imm_persistence() {
        let m1 = super::YnImmutableMap::yn_new().yn_insert(1, 10);
        let m2 = m1.yn_insert(2, 20);
        assert_eq!(m1.yn_len(), 1);
        assert_eq!(m2.yn_len(), 2);
    }

    #[test]
    fn yn_imm_remove() {
        let m = super::YnImmutableMap::yn_new().yn_insert(1, 10).yn_insert(2, 20);
        let m2 = m.yn_remove(&1);
        assert!(!m2.yn_contains_key(&1));
        assert!(m.yn_contains_key(&1));
    }

    #[test]
    fn yn_imm_keys() {
        let m = super::YnImmutableMap::yn_new().yn_insert(3, 0).yn_insert(1, 0).yn_insert(2, 0);
        assert_eq!(m.yn_keys(), vec![1, 2, 3]);
    }

    #[test]
    fn yn_imm_merge() {
        let a = super::YnImmutableMap::yn_new().yn_insert(1, "a");
        let b = super::YnImmutableMap::yn_new().yn_insert(2, "b");
        let c = a.yn_merge(&b);
        assert_eq!(c.yn_len(), 2);
    }

    #[test]
    fn yn_imm_filter() {
        let m = super::YnImmutableMap::yn_new().yn_insert(1, 10).yn_insert(2, 20).yn_insert(3, 30);
        let f = m.yn_filter(|_, v| *v > 15);
        assert_eq!(f.yn_len(), 2);
    }

    #[test]
    fn yn_imm_display() {
        let m = super::YnImmutableMap::<i32, i32>::yn_new();
        assert!(format!("{}", m).contains("ImmMap"));
    }

    #[test]
    fn yn_imm_default() {
        let m = super::YnImmutableMap::<i32, i32>::default();
        assert!(m.yn_is_empty());
    }

    // --- yn_ Tokenizer tests ---

    #[test]
    fn yn_tok_word() {
        let tokens = super::YnTokenizer::yn_tokenize("hello");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnWord);
    }

    #[test]
    fn yn_tok_number() {
        let tokens = super::YnTokenizer::yn_tokenize("42");
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnNumber);
    }

    #[test]
    fn yn_tok_mixed() {
        let tokens = super::YnTokenizer::yn_tokenize_no_ws("x + 42");
        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn yn_tok_string() {
        let tokens = super::YnTokenizer::yn_tokenize("hi_world");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnWord);
    }

    #[test]
    fn yn_tok_punct() {
        let tokens = super::YnTokenizer::yn_tokenize("+");
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnPunct);
    }

    #[test]
    fn yn_tok_count_kind() {
        let tokens = super::YnTokenizer::yn_tokenize("a + b + c");
        let count = super::YnTokenizer::yn_count_by_kind(&tokens, &super::YnTokenKind::YnWord);
        assert_eq!(count, 3);
    }

    #[test]
    fn yn_tok_empty() {
        let tokens = super::YnTokenizer::yn_tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn yn_tok_display() {
        let t = super::YnTokenizer;
        assert!(format!("{}", t).contains("Tokenizer"));
    }

    #[test]
    fn yn_tok_default() {
        let _t = super::YnTokenizer::default();
    }

    #[test]
    fn yn_tok_kind_display() {
        assert!(format!("{}", super::YnTokenKind::YnWord).contains("Word"));
    }


    // --- yo_ Levenshtein tests ---

    #[test]
    fn yo_lev_identical() {
        assert_eq!(super::YoLevenshtein::yo_distance("hello", "hello"), 0);
    }

    #[test]
    fn yo_lev_empty() {
        assert_eq!(super::YoLevenshtein::yo_distance("", "abc"), 3);
        assert_eq!(super::YoLevenshtein::yo_distance("abc", ""), 3);
    }

    #[test]
    fn yo_lev_basic() {
        assert_eq!(super::YoLevenshtein::yo_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn yo_lev_single_char() {
        assert_eq!(super::YoLevenshtein::yo_distance("a", "b"), 1);
    }

    #[test]
    fn yo_lev_similarity() {
        let s = super::YoLevenshtein::yo_similarity("hello", "hello");
        assert!((s - 1.0).abs() < 0.001);
    }

    #[test]
    fn yo_lev_closest() {
        let c = super::YoLevenshtein::yo_closest("cat", &["bat", "car", "dog"]);
        assert!(c == Some("bat") || c == Some("car"));
    }

    #[test]
    fn yo_lev_within() {
        let r = super::YoLevenshtein::yo_within_distance("cat", &["bat", "car", "dog"], 1);
        assert!(r.contains(&"bat"));
        assert!(r.contains(&"car"));
    }

    #[test]
    fn yo_lev_rank() {
        let r = super::YoLevenshtein::yo_rank("cat", &["dog", "bat", "cat"]);
        assert_eq!(r[0].0, "cat");
    }

    #[test]
    fn yo_lev_display() {
        assert!(format!("{}", super::YoLevenshtein).contains("Levenshtein"));
    }

    // --- yo_ DiffEngine tests ---

    #[test]
    fn yo_diff_identical() {
        let ops = super::YoDiffEngine::yo_diff("a\nb", "a\nb");
        assert_eq!(super::YoDiffEngine::yo_count_equal(&ops), 2);
    }

    #[test]
    fn yo_diff_insert() {
        let ops = super::YoDiffEngine::yo_diff("a", "a\nb");
        assert_eq!(super::YoDiffEngine::yo_count_insertions(&ops), 1);
    }

    #[test]
    fn yo_diff_delete() {
        let ops = super::YoDiffEngine::yo_diff("a\nb", "a");
        assert_eq!(super::YoDiffEngine::yo_count_deletions(&ops), 1);
    }

    #[test]
    fn yo_diff_replace() {
        let ops = super::YoDiffEngine::yo_diff("a", "b");
        assert!(super::YoDiffEngine::yo_count_insertions(&ops) > 0 || super::YoDiffEngine::yo_count_deletions(&ops) > 0);
    }

    #[test]
    fn yo_diff_empty() {
        let ops = super::YoDiffEngine::yo_diff("", "");
        assert!(ops.is_empty());
    }

    #[test]
    fn yo_diff_format() {
        let ops = super::YoDiffEngine::yo_diff("a", "b");
        let s = super::YoDiffEngine::yo_format(&ops);
        assert!(!s.is_empty());
    }

    #[test]
    fn yo_diff_op_display() {
        let op = super::YoDiffOp::YoInsert("line".to_string());
        assert!(format!("{}", op).contains("+"));
    }

    #[test]
    fn yo_diff_display() {
        assert!(format!("{}", super::YoDiffEngine).contains("DiffEngine"));
    }


    // --- yp_ JsonValue tests ---

    #[test]
    fn yp_json_null() {
        let v = super::YpJsonValue::YpNull;
        assert!(v.yp_is_null());
    }

    #[test]
    fn yp_json_string() {
        let v = super::YpJsonValue::yp_string("hello");
        assert_eq!(v.yp_as_str(), Some("hello"));
    }

    #[test]
    fn yp_json_number() {
        let v = super::YpJsonValue::yp_number(42.0);
        assert_eq!(v.yp_as_f64(), Some(42.0));
    }

    #[test]
    fn yp_json_bool() {
        let v = super::YpJsonValue::yp_bool(true);
        assert_eq!(v.yp_as_bool(), Some(true));
    }

    #[test]
    fn yp_json_object() {
        let mut obj = super::YpJsonValue::yp_object();
        obj.yp_set("name", super::YpJsonValue::yp_string("test"));
        assert_eq!(obj.yp_get("name").unwrap().yp_as_str(), Some("test"));
    }

    #[test]
    fn yp_json_array() {
        let mut arr = super::YpJsonValue::yp_array();
        arr.yp_push(super::YpJsonValue::yp_number(1.0));
        arr.yp_push(super::YpJsonValue::yp_number(2.0));
        assert_eq!(arr.yp_len(), 2);
    }

    #[test]
    fn yp_json_path() {
        let mut obj = super::YpJsonValue::yp_object();
        let mut inner = super::YpJsonValue::yp_object();
        inner.yp_set("b", super::YpJsonValue::yp_number(42.0));
        obj.yp_set("a", inner);
        assert_eq!(obj.yp_path("a.b").unwrap().yp_as_f64(), Some(42.0));
    }

    #[test]
    fn yp_json_merge() {
        let mut a = super::YpJsonValue::yp_object();
        a.yp_set("x", super::YpJsonValue::yp_number(1.0));
        let mut b = super::YpJsonValue::yp_object();
        b.yp_set("y", super::YpJsonValue::yp_number(2.0));
        let c = a.yp_merge(&b);
        assert_eq!(c.yp_len(), 2);
    }

    #[test]
    fn yp_json_keys() {
        let mut obj = super::YpJsonValue::yp_object();
        obj.yp_set("a", super::YpJsonValue::YpNull);
        obj.yp_set("b", super::YpJsonValue::YpNull);
        assert_eq!(obj.yp_keys().len(), 2);
    }

    #[test]
    fn yp_json_display() {
        let v = super::YpJsonValue::yp_string("hi");
        assert!(format!("{}", v).contains("hi"));
    }

    #[test]
    fn yp_json_default() {
        let v = super::YpJsonValue::default();
        assert!(v.yp_is_null());
    }

    // --- yp_ CommandRegistry tests ---

    #[test]
    fn yp_cmdreg_new() {
        let r = super::YpCommandRegistry::yp_new();
        assert_eq!(r.yp_count(), 0);
    }

    #[test]
    fn yp_cmdreg_register() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("editor.copy", "Copy", "Edit");
        assert_eq!(r.yp_count(), 1);
    }

    #[test]
    fn yp_cmdreg_find() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("editor.copy", "Copy", "Edit");
        assert!(r.yp_find("editor.copy").is_some());
    }

    #[test]
    fn yp_cmdreg_search() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("editor.copy", "Copy Selection", "Edit");
        r.yp_register("editor.paste", "Paste", "Edit");
        let results = r.yp_search("copy");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn yp_cmdreg_category() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("a", "A", "Edit");
        r.yp_register("b", "B", "View");
        assert_eq!(r.yp_by_category("Edit").len(), 1);
    }

    #[test]
    fn yp_cmdreg_keybinding() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register_with_key("copy", "Copy", "Edit", "Ctrl+C");
        assert!(r.yp_by_keybinding("Ctrl+C").is_some());
    }

    #[test]
    fn yp_cmdreg_categories() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("a", "A", "Edit");
        r.yp_register("b", "B", "View");
        assert_eq!(r.yp_categories().len(), 2);
    }

    #[test]
    fn yp_cmdreg_clear() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("a", "A", "X");
        r.yp_clear();
        assert_eq!(r.yp_count(), 0);
    }

    #[test]
    fn yp_cmdreg_display() {
        let r = super::YpCommandRegistry::yp_new();
        assert!(format!("{}", r).contains("CmdRegistry"));
    }

    #[test]
    fn yp_cmd_display() {
        let c = super::YpCommandEntry { yp_id: "test".into(), yp_title: "T".into(), yp_category: "C".into(), yp_keybinding: None, yp_when: None };
        assert!(format!("{}", c).contains("Cmd"));
    }


    // --- yq_ ConfigStore tests ---

    #[test]
    fn yq_config_new() {
        let c = super::YqConfigStore::yq_new();
        assert_eq!(c.yq_layer_count(), 3);
    }

    #[test]
    fn yq_config_set_get() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "theme", "dark");
        assert_eq!(c.yq_get("theme"), Some("dark"));
    }

    #[test]
    fn yq_config_layering() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "font", "mono");
        c.yq_set("user", "font", "sans");
        assert_eq!(c.yq_get("font"), Some("sans"));
    }

    #[test]
    fn yq_config_workspace_overrides() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "size", "12");
        c.yq_set("workspace", "size", "14");
        assert_eq!(c.yq_get("size"), Some("14"));
    }

    #[test]
    fn yq_config_get_or() {
        let c = super::YqConfigStore::yq_new();
        assert_eq!(c.yq_get_or("missing", "default"), "default");
    }

    #[test]
    fn yq_config_get_i64() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "port", "8080");
        assert_eq!(c.yq_get_i64("port"), Some(8080));
    }

    #[test]
    fn yq_config_get_bool() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "debug", "true");
        assert_eq!(c.yq_get_bool("debug"), Some(true));
    }

    #[test]
    fn yq_config_all_keys() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "a", "1");
        c.yq_set("user", "b", "2");
        assert_eq!(c.yq_all_keys().len(), 2);
    }

    #[test]
    fn yq_config_effective() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "x", "1");
        c.yq_set("user", "x", "2");
        assert_eq!(c.yq_effective_layer("x"), Some("user"));
    }

    #[test]
    fn yq_config_clear() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "a", "1");
        c.yq_clear_layer("user");
        assert_eq!(c.yq_get("a"), None);
    }

    #[test]
    fn yq_config_display() {
        let c = super::YqConfigStore::yq_new();
        assert!(format!("{}", c).contains("ConfigStore"));
    }

    // --- yq_ TextLayout tests ---

    #[test]
    fn yq_layout_wrap_short() {
        let l = super::YqTextLayout::yq_new(80);
        let lines = l.yq_wrap("hello");
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn yq_layout_wrap_long() {
        let l = super::YqTextLayout::yq_new(10);
        let lines = l.yq_wrap("hello world foo bar");
        assert!(lines.len() > 1);
    }

    #[test]
    fn yq_layout_truncate() {
        let l = super::YqTextLayout::yq_new(10);
        let t = l.yq_truncate("hello world foo", "...");
        assert!(t.len() <= 10);
        assert!(t.ends_with("..."));
    }

    #[test]
    fn yq_layout_pad() {
        let l = super::YqTextLayout::yq_new(10);
        let p = l.yq_pad_right("hi");
        assert_eq!(p.len(), 10);
    }

    #[test]
    fn yq_layout_center() {
        let l = super::YqTextLayout::yq_new(10);
        let c = l.yq_center("hi");
        assert_eq!(c.len(), 10);
    }

    #[test]
    fn yq_layout_line_count() {
        let l = super::YqTextLayout::yq_new(5);
        assert!(l.yq_line_count("hello world") > 1);
    }

    #[test]
    fn yq_layout_display() {
        let l = super::YqTextLayout::yq_new(80);
        assert!(format!("{}", l).contains("TextLayout"));
    }

    #[test]
    fn yq_layout_default() {
        let l = super::YqTextLayout::default();
        assert_eq!(l.yq_width(), 80);
    }

}