//! URI comparison and normalization.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur when parsing a URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
    InvalidUri,
    MissingScheme,
    EmptyPath,
}

impl fmt::Display for UriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UriError::InvalidUri => write!(f, "invalid URI"),
            UriError::MissingScheme => write!(f, "missing scheme"),
            UriError::EmptyPath => write!(f, "empty path"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceUri {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl ResourceUri {
    pub fn new(scheme: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            authority: String::new(),
            path: path.into(),
            query: None,
            fragment: None,
        }
    }

    pub fn file(path: impl Into<String>) -> Self {
        Self::new("file", path)
    }

    pub fn from_string(uri_str: &str) -> Option<Self> {
        let (scheme, rest) = uri_str.split_once("://")?;
        let (authority_and_path, fragment) = match rest.rsplit_once('#') {
            Some((ap, f)) => (ap, Some(f.to_string())),
            None => (rest, None),
        };
        let (authority_and_path, query) = match authority_and_path.split_once('?') {
            Some((ap, q)) => (ap, Some(q.to_string())),
            None => (authority_and_path, None),
        };
        let (authority, path) = if scheme == "file" {
            (String::new(), authority_and_path.to_string())
        } else {
            match authority_and_path.find('/') {
                Some(idx) => (
                    authority_and_path[..idx].to_string(),
                    authority_and_path[idx..].to_string(),
                ),
                None => (authority_and_path.to_string(), String::new()),
            }
        };
        Some(Self {
            scheme: scheme.to_string(),
            authority,
            path,
            query,
            fragment,
        })
    }

    pub fn to_string(&self) -> String {
        let mut s = format!("{}://{}{}", self.scheme, self.authority, self.path);
        if let Some(q) = &self.query {
            s.push('?');
            s.push_str(q);
        }
        if let Some(f) = &self.fragment {
            s.push('#');
            s.push_str(f);
        }
        s
    }

    pub fn filename(&self) -> Option<&str> {
        self.path.rsplit('/').next().filter(|s| !s.is_empty())
    }

    pub fn extension(&self) -> Option<&str> {
        self.filename()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, ext)| ext)
    }

    /// Parse a URI string, returning a descriptive error on failure.
    pub fn try_parse(uri_str: &str) -> Result<Self, UriError> {
        let (scheme, rest) = uri_str
            .split_once("://")
            .ok_or(UriError::MissingScheme)?;
        if scheme.is_empty() {
            return Err(UriError::MissingScheme);
        }
        let (authority_and_path, fragment) = match rest.rsplit_once('#') {
            Some((ap, f)) => (ap, Some(f.to_string())),
            None => (rest, None),
        };
        let (authority_and_path, query) = match authority_and_path.split_once('?') {
            Some((ap, q)) => (ap, Some(q.to_string())),
            None => (authority_and_path, None),
        };
        let (authority, path) = if scheme == "file" {
            (String::new(), authority_and_path.to_string())
        } else {
            match authority_and_path.find('/') {
                Some(idx) => (
                    authority_and_path[..idx].to_string(),
                    authority_and_path[idx..].to_string(),
                ),
                None => (authority_and_path.to_string(), String::new()),
            }
        };
        if path.is_empty() && scheme == "file" {
            return Err(UriError::EmptyPath);
        }
        Ok(Self {
            scheme: scheme.to_string(),
            authority,
            path,
            query,
            fragment,
        })
    }

    /// Returns the parent URI (directory) by stripping the last path segment.
    pub fn parent(&self) -> Option<Self> {
        let trimmed = self.path.trim_end_matches('/');
        let idx = trimmed.rfind('/')?;
        if idx == 0 && trimmed.len() == 1 {
            return None;
        }
        let parent_path = if idx == 0 {
            "/".to_string()
        } else {
            trimmed[..idx].to_string()
        };
        Some(Self {
            scheme: self.scheme.clone(),
            authority: self.authority.clone(),
            path: parent_path,
            query: None,
            fragment: None,
        })
    }

    /// Builder-style method to set the fragment.
    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        self.fragment = Some(fragment.into());
        self
    }

    /// Builder-style method to set the query string.
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Returns true if the scheme is `"file"`.
    pub fn is_file(&self) -> bool {
        self.scheme == "file"
    }
}

impl fmt::Display for ResourceUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

pub fn are_equal(a: &ResourceUri, b: &ResourceUri) -> bool {
    a.scheme.eq_ignore_ascii_case(&b.scheme)
        && a.authority.eq_ignore_ascii_case(&b.authority)
        && a.path.eq_ignore_ascii_case(&b.path)
        && a.query == b.query
        && a.fragment == b.fragment
}

pub struct UriIdentityService {
    mappings: HashMap<String, ResourceUri>,
}

impl UriIdentityService {
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    pub fn register(&mut self, key: impl Into<String>, uri: ResourceUri) {
        self.mappings.insert(key.into(), uri);
    }

    pub fn resolve(&self, key: &str) -> Option<&ResourceUri> {
        self.mappings.get(key)
    }

    /// Remove a mapping by key, returning the previously stored URI if present.
    pub fn unregister(&mut self, key: &str) -> Option<ResourceUri> {
        self.mappings.remove(key)
    }

    /// Resolve a key, falling back to parsing it as a URI string.
    pub fn resolve_or_parse(&self, key: &str) -> Option<ResourceUri> {
        self.mappings
            .get(key)
            .cloned()
            .or_else(|| ResourceUri::from_string(key))
    }

    /// Returns the number of registered mappings.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Returns true if no mappings are registered.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Removes all registered mappings.
    pub fn clear(&mut self) {
        self.mappings.clear();
    }
}

impl Default for UriIdentityService {
    fn default() -> Self {
        Self::new()
    }
}

/// Percent-encode a URI path component, encoding characters outside unreserved set.
pub fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/' => out.push(byte as char),
            _ => {
                out.push('%');
                out.push(char::from(HEX_CHARS[(byte >> 4) as usize]));
                out.push(char::from(HEX_CHARS[(byte & 0x0F) as usize]));
            }
        }
    }
    out
}

const HEX_CHARS: [u8; 16] = *b"0123456789ABCDEF";

/// Decode a percent-encoded URI path component.
pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// Accumulated statistics for uriidentity operations.
#[derive(Debug, Clone, PartialEq)]
pub struct UriidentityStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl UriidentityStats {
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
    pub fn merge(&mut self, other: &UriidentityStats) {
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

impl Default for UriidentityStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UriidentityStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UriidentityStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for uriidentity.
#[derive(Debug, Clone)]
pub struct UriidentityValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl UriidentityValidator {
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

impl Default for UriidentityValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// UriPattern — matching URIs with glob patterns
// ---------------------------------------------------------------------------

/// A pattern for matching URIs using glob-style wildcards in the path.
///
/// Supports:
/// - `*` matches any characters within a single path segment
/// - `**` matches any number of path segments
/// - `?` matches a single character
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriPattern {
    pub scheme: Option<String>,
    pub authority: Option<String>,
    pub path_pattern: String,
}

impl UriPattern {
    /// Create a pattern matching any scheme/authority with the given path glob.
    pub fn path_only(pattern: impl Into<String>) -> Self {
        Self {
            scheme: None,
            authority: None,
            path_pattern: pattern.into(),
        }
    }

    /// Create a pattern for a specific scheme.
    pub fn with_scheme(scheme: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            scheme: Some(scheme.into()),
            authority: None,
            path_pattern: pattern.into(),
        }
    }

    /// Create a fully specified pattern.
    pub fn full(
        scheme: impl Into<String>,
        authority: impl Into<String>,
        pattern: impl Into<String>,
    ) -> Self {
        Self {
            scheme: Some(scheme.into()),
            authority: Some(authority.into()),
            path_pattern: pattern.into(),
        }
    }

    /// Test if a URI matches this pattern.
    pub fn matches(&self, uri: &ResourceUri) -> bool {
        if let Some(ref s) = self.scheme {
            if !s.eq_ignore_ascii_case(&uri.scheme) {
                return false;
            }
        }
        if let Some(ref a) = self.authority {
            if !a.eq_ignore_ascii_case(&uri.authority) {
                return false;
            }
        }
        glob_match(&self.path_pattern, &uri.path)
    }

    /// Test if a URI string matches this pattern.
    pub fn matches_str(&self, uri_str: &str) -> bool {
        if let Some(uri) = ResourceUri::from_string(uri_str) {
            self.matches(&uri)
        } else {
            false
        }
    }
}

impl fmt::Display for UriPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref s) = self.scheme {
            write!(f, "{}://", s)?;
        }
        if let Some(ref a) = self.authority {
            write!(f, "{}", a)?;
        }
        write!(f, "{}", self.path_pattern)
    }
}

/// Simple glob matching for paths.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('/').collect();
    let text_parts: Vec<&str> = text.split('/').collect();
    glob_match_parts(&pat_parts, &text_parts)
}

fn glob_match_parts(pat_parts: &[&str], text_parts: &[&str]) -> bool {
    if pat_parts.is_empty() {
        return text_parts.is_empty();
    }
    if pat_parts[0] == "**" {
        // Match zero or more segments
        for i in 0..=text_parts.len() {
            if glob_match_parts(&pat_parts[1..], &text_parts[i..]) {
                return true;
            }
        }
        return false;
    }
    if text_parts.is_empty() {
        return false;
    }
    if segment_matches(pat_parts[0], text_parts[0]) {
        glob_match_parts(&pat_parts[1..], &text_parts[1..])
    } else {
        false
    }
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let pat_bytes = pattern.as_bytes();
    let text_bytes = text.as_bytes();
    segment_match_dp(pat_bytes, text_bytes)
}

fn segment_match_dp(pat: &[u8], text: &[u8]) -> bool {
    let (m, n) = (pat.len(), text.len());
    let mut dp = vec![vec![false; n + 1]; m + 1];
    dp[0][0] = true;
    for i in 1..=m {
        if pat[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=m {
        for j in 1..=n {
            if pat[i - 1] == b'*' {
                dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
            } else if pat[i - 1] == b'?' || pat[i - 1] == text[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            }
        }
    }
    dp[m][n]
}

// ---------------------------------------------------------------------------
// URI normalization helpers
// ---------------------------------------------------------------------------

/// Normalize a URI by applying standard transformations.
pub fn normalize_uri(uri: &ResourceUri) -> ResourceUri {
    let mut path = uri.path.replace('\\', "/");
    // Remove trailing slash unless root
    if path.len() > 1 && path.ends_with('/') {
        path.truncate(path.len() - 1);
    }
    // Normalize percent encoding
    path = normalize_percent_encoding(&path);
    // Remove duplicate slashes
    while path.contains("//") {
        path = path.replace("//", "/");
    }
    ResourceUri {
        scheme: uri.scheme.to_lowercase(),
        authority: uri.authority.to_lowercase(),
        path,
        query: uri.query.clone(),
        fragment: uri.fragment.clone(),
    }
}

/// Add a trailing slash to the URI path if not already present.
pub fn ensure_trailing_slash(uri: &ResourceUri) -> ResourceUri {
    let mut result = uri.clone();
    if !result.path.ends_with('/') {
        result.path.push('/');
    }
    result
}

/// Remove the trailing slash from the URI path (unless it's the root "/").
pub fn remove_trailing_slash(uri: &ResourceUri) -> ResourceUri {
    let mut result = uri.clone();
    if result.path.len() > 1 && result.path.ends_with('/') {
        result.path.pop();
    }
    result
}

/// Join a base URI with a relative path.
pub fn uri_join(base: &ResourceUri, relative: &str) -> ResourceUri {
    let mut base_path = base.path.clone();
    if !base_path.ends_with('/') {
        // Remove last segment to get directory
        if let Some(idx) = base_path.rfind('/') {
            base_path.truncate(idx + 1);
        }
    }

    let mut segments: Vec<&str> = base_path.split('/').filter(|s| !s.is_empty()).collect();

    for part in relative.split('/') {
        match part {
            "." | "" => {}
            ".." => { segments.pop(); }
            other => segments.push(other),
        }
    }

    let joined = format!("/{}", segments.join("/"));
    ResourceUri {
        scheme: base.scheme.clone(),
        authority: base.authority.clone(),
        path: joined,
        query: None,
        fragment: None,
    }
}

/// Compute the depth (number of path segments) of a URI.
pub fn uri_depth(uri: &ResourceUri) -> usize {
    uri.path.split('/').filter(|s| !s.is_empty()).count()
}

/// Returns true if `child` is a descendant of `parent` (its path starts with parent's path).
pub fn uri_is_child(parent: &ResourceUri, child: &ResourceUri) -> bool {
    if !parent.scheme.eq_ignore_ascii_case(&child.scheme)
        || !parent.authority.eq_ignore_ascii_case(&child.authority)
    {
        return false;
    }
    let parent_path = if parent.path.ends_with('/') {
        parent.path.clone()
    } else {
        format!("{}/", parent.path)
    };
    child.path.starts_with(&parent_path) && child.path.len() > parent_path.len()
}

/// Normalizer for URI identity comparison, supporting case-insensitive matching.
#[derive(Debug, Clone)]
pub struct UriIdentityNormalizer {
    /// Whether to ignore case when comparing paths (Windows-style).
    pub ignore_path_case: bool,
    /// Whether to normalize path separators (backslash to forward slash).
    pub normalize_separators: bool,
    /// Whether to remove trailing slashes.
    pub strip_trailing_slash: bool,
}

impl UriIdentityNormalizer {
    pub fn new() -> Self {
        Self {
            ignore_path_case: false,
            normalize_separators: true,
            strip_trailing_slash: true,
        }
    }

    /// Create a normalizer for case-insensitive file systems (Windows).
    pub fn windows() -> Self {
        Self {
            ignore_path_case: true,
            normalize_separators: true,
            strip_trailing_slash: true,
        }
    }

    /// Create a normalizer for case-sensitive file systems (Linux/macOS).
    pub fn unix() -> Self {
        Self {
            ignore_path_case: false,
            normalize_separators: false,
            strip_trailing_slash: true,
        }
    }

    /// Normalize a path string according to this normalizer's settings.
    pub fn normalize_path(&self, path: &str) -> String {
        let mut result = path.to_string();
        if self.normalize_separators {
            result = result.replace('\\', "/");
        }
        if self.strip_trailing_slash && result.len() > 1 {
            result = result.trim_end_matches('/').to_string();
            if result.is_empty() {
                result = "/".to_string();
            }
        }
        if self.ignore_path_case {
            result = result.to_lowercase();
        }
        result
    }

    /// Compare two URIs for identity using this normalizer's settings.
    pub fn are_equal(&self, a: &ResourceUri, b: &ResourceUri) -> bool {
        let scheme_eq = a.scheme.eq_ignore_ascii_case(&b.scheme);
        let auth_eq = a.authority.eq_ignore_ascii_case(&b.authority);
        let path_a = self.normalize_path(&a.path);
        let path_b = self.normalize_path(&b.path);
        scheme_eq && auth_eq && path_a == path_b && a.query == b.query && a.fragment == b.fragment
    }
}

impl Default for UriIdentityNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Produce a canonical string representation of a URI.
/// - Scheme and authority are lowercased.
/// - Path separators are normalized to forward slashes.
/// - Trailing slashes are removed (except root "/").
/// - Percent-encoding is normalized to uppercase hex.
pub fn uri_canonical_form(uri: &ResourceUri) -> String {
    let scheme = uri.scheme.to_lowercase();
    let authority = uri.authority.to_lowercase();
    let mut path = uri.path.replace('\\', "/");
    // Remove trailing slash unless it's the root
    if path.len() > 1 && path.ends_with('/') {
        path.truncate(path.len() - 1);
    }
    // Normalize percent encoding to uppercase
    path = normalize_percent_encoding(&path);
    let mut result = format!("{}://{}{}", scheme, authority, path);
    if let Some(ref q) = uri.query {
        result.push('?');
        result.push_str(q);
    }
    if let Some(ref f) = uri.fragment {
        result.push('#');
        result.push_str(f);
    }
    result
}

/// Normalize percent-encoded characters to uppercase hex digits.
fn normalize_percent_encoding(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = bytes[i + 1];
            let lo = bytes[i + 2];
            if hi.is_ascii_hexdigit() && lo.is_ascii_hexdigit() {
                out.push('%');
                out.push((hi as char).to_ascii_uppercase());
                out.push((lo as char).to_ascii_uppercase());
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Compute the relative path from `base` to `target`.
/// Both URIs must have the same scheme and authority.
/// Returns `None` if the URIs are not in the same origin.
pub fn uri_relative_path(base: &ResourceUri, target: &ResourceUri) -> Option<String> {
    if !base.scheme.eq_ignore_ascii_case(&target.scheme)
        || !base.authority.eq_ignore_ascii_case(&target.authority)
    {
        return None;
    }

    let base_parts: Vec<&str> = base.path.split('/').filter(|s| !s.is_empty()).collect();
    let target_parts: Vec<&str> = target.path.split('/').filter(|s| !s.is_empty()).collect();

    // Find common prefix length
    let common = base_parts
        .iter()
        .zip(target_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ups = base_parts.len().saturating_sub(common);
    let mut parts: Vec<&str> = Vec::new();
    for _ in 0..ups {
        parts.push("..");
    }
    for part in &target_parts[common..] {
        parts.push(part);
    }

    if parts.is_empty() {
        Some(".".to_string())
    } else {
        Some(parts.join("/"))
    }
}

/// Computes a simple FNV-1a hash of the URI's canonical form for use as an identity key.
pub fn uri_identity_hash(uri: &ResourceUri) -> u64 {
    let canonical = uri_canonical_form(uri);
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in canonical.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Returns true if two URIs refer to the same document (same scheme, authority, and path,
/// ignoring query and fragment differences).
pub fn uri_same_document(a: &ResourceUri, b: &ResourceUri) -> bool {
    a.scheme.eq_ignore_ascii_case(&b.scheme)
        && a.authority.eq_ignore_ascii_case(&b.authority)
        && a.path == b.path
}

/// Decomposes a URI string into its components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriComponents {
    pub scheme: Option<String>,
    pub authority: Option<String>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl UriComponents {
    /// Parses a URI string into components (basic parsing: split on "://", "?", "#").
    pub fn parse(uri_str: &str) -> Self {
        let (scheme, after_scheme) = if let Some(idx) = uri_str.find("://") {
            (Some(uri_str[..idx].to_string()), &uri_str[idx + 3..])
        } else {
            (None, uri_str)
        };

        let (rest, fragment) = if let Some(idx) = after_scheme.find('#') {
            (&after_scheme[..idx], Some(after_scheme[idx + 1..].to_string()))
        } else {
            (after_scheme, None)
        };

        let (rest, query) = if let Some(idx) = rest.find('?') {
            (&rest[..idx], Some(rest[idx + 1..].to_string()))
        } else {
            (rest, None)
        };

        let (authority, path) = if scheme.is_some() {
            if let Some(idx) = rest.find('/') {
                let auth = &rest[..idx];
                (
                    if auth.is_empty() { None } else { Some(auth.to_string()) },
                    rest[idx..].to_string(),
                )
            } else if rest.is_empty() {
                (None, String::new())
            } else {
                (Some(rest.to_string()), String::new())
            }
        } else {
            (None, rest.to_string())
        };

        Self {
            scheme,
            authority,
            path,
            query,
            fragment,
        }
    }

    /// Converts to a ResourceUri.
    pub fn to_resource_uri(&self) -> ResourceUri {
        ResourceUri {
            scheme: self.scheme.clone().unwrap_or_default(),
            authority: self.authority.clone().unwrap_or_default(),
            path: self.path.clone(),
            query: self.query.clone(),
            fragment: self.fragment.clone(),
        }
    }

    pub fn has_authority(&self) -> bool {
        self.authority.is_some()
    }

    pub fn has_query(&self) -> bool {
        self.query.is_some()
    }

    pub fn has_fragment(&self) -> bool {
        self.fragment.is_some()
    }
}

// ---------------------------------------------------------------------------
// URI analysis and batch utilities
// ---------------------------------------------------------------------------

/// Return all unique schemes present in a collection of URIs.
pub fn uri_unique_schemes(uris: &[ResourceUri]) -> Vec<String> {
    let mut schemes: Vec<String> = uris.iter().map(|u| u.scheme.clone()).collect();
    schemes.sort();
    schemes.dedup();
    schemes
}

/// Group URIs by their scheme.
pub fn uri_group_by_scheme(uris: &[ResourceUri]) -> HashMap<String, Vec<&ResourceUri>> {
    let mut groups: HashMap<String, Vec<&ResourceUri>> = HashMap::new();
    for uri in uris {
        groups.entry(uri.scheme.clone()).or_default().push(uri);
    }
    groups
}

/// Return URIs sorted by path depth (shallowest first).
pub fn uri_sort_by_depth(uris: &mut [ResourceUri]) {
    uris.sort_by_key(|u| uri_depth(u));
}

/// Find all URIs that are children of the given parent URI.
pub fn uri_find_children<'a>(parent: &ResourceUri, uris: &'a [ResourceUri]) -> Vec<&'a ResourceUri> {
    uris.iter().filter(|u| uri_is_child(parent, u)).collect()
}

/// Return the common path prefix among a set of file URIs.
pub fn uri_common_path(uris: &[ResourceUri]) -> String {
    if uris.is_empty() {
        return String::new();
    }
    let paths: Vec<&str> = uris.iter().map(|u| u.path.as_str()).collect();
    let first = paths[0];
    let mut prefix_len = first.len();
    for p in &paths[1..] {
        prefix_len = first
            .chars()
            .zip(p.chars())
            .take(prefix_len)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let prefix = &first[..first.char_indices().nth(prefix_len).map(|(i, _)| i).unwrap_or(first.len())];
    match prefix.rfind('/') {
        Some(idx) => prefix[..=idx].to_string(),
        None => String::new(),
    }
}

/// Return `true` if two URIs share the same scheme and authority.
pub fn uri_same_origin(a: &ResourceUri, b: &ResourceUri) -> bool {
    a.scheme == b.scheme && a.authority == b.authority
}

/// Return `true` if the URI path has the given file extension (case-insensitive).
pub fn uri_has_extension(uri: &ResourceUri, ext: &str) -> bool {
    uri.extension()
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// URI path manipulation utilities
// ---------------------------------------------------------------------------

/// Return the directory portion of the URI path (everything before the last `/`).
pub fn uri_dirname(uri: &ResourceUri) -> String {
    match uri.path.rfind('/') {
        Some(pos) if pos > 0 => uri.path[..pos].to_string(),
        _ => "/".to_string(),
    }
}

/// Return the base name (last path segment) of the URI.
pub fn uri_basename(uri: &ResourceUri) -> &str {
    uri.path.rsplit('/').next().unwrap_or(&uri.path)
}

/// Return the URI path with its extension replaced.
pub fn uri_with_extension(uri: &ResourceUri, new_ext: &str) -> ResourceUri {
    let new_path = match uri.path.rfind('.') {
        Some(pos) => format!("{}.{}", &uri.path[..pos], new_ext),
        None => format!("{}.{}", uri.path, new_ext),
    };
    ResourceUri {
        scheme: uri.scheme.clone(),
        authority: uri.authority.clone(),
        path: new_path,
        query: uri.query.clone(),
        fragment: uri.fragment.clone(),
    }
}

/// Append a path segment to a URI (ensures single `/` separator).
pub fn uri_append_path(uri: &ResourceUri, segment: &str) -> ResourceUri {
    let base = uri.path.trim_end_matches('/');
    let seg = segment.trim_start_matches('/');
    ResourceUri {
        scheme: uri.scheme.clone(),
        authority: uri.authority.clone(),
        path: format!("{base}/{seg}"),
        query: uri.query.clone(),
        fragment: uri.fragment.clone(),
    }
}

/// Count the number of path segments in the URI path.
pub fn uri_segment_count(uri: &ResourceUri) -> usize {
    uri.path
        .split('/')
        .filter(|s| !s.is_empty())
        .count()
}

/// Return true if the URI path is at the root level (single segment or empty).
pub fn uri_is_root(uri: &ResourceUri) -> bool {
    uri_segment_count(uri) <= 1
}

/// Return the path segments as a vector of string slices.
pub fn uri_path_segments(uri: &ResourceUri) -> Vec<&str> {
    uri.path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Return a URI with query parameters set from a key-value map.
pub fn uri_with_query(uri: &ResourceUri, params: &[(&str, &str)]) -> ResourceUri {
    let query_str = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    ResourceUri {
        scheme: uri.scheme.clone(),
        authority: uri.authority.clone(),
        path: uri.path.clone(),
        query: if query_str.is_empty() { None } else { Some(query_str) },
        fragment: uri.fragment.clone(),
    }
}

/// Return a URI with the fragment set.
pub fn uri_with_fragment(uri: &ResourceUri, fragment: &str) -> ResourceUri {
    ResourceUri {
        scheme: uri.scheme.clone(),
        authority: uri.authority.clone(),
        path: uri.path.clone(),
        query: uri.query.clone(),
        fragment: if fragment.is_empty() { None } else { Some(fragment.to_string()) },
    }
}

/// Strip query and fragment from a URI, returning a clean path-only URI.
pub fn uri_strip_query_fragment(uri: &ResourceUri) -> ResourceUri {
    ResourceUri {
        scheme: uri.scheme.clone(),
        authority: uri.authority.clone(),
        path: uri.path.clone(),
        query: None,
        fragment: None,
    }
}

/// Canonicalizes URIs for identity comparison by lowercasing scheme/authority,
/// decoding unnecessary percent-encoding, normalizing path separators, and
/// removing trailing slashes.
pub struct UriCanonicalizer {
    normalize_case: bool,
    strip_trailing_slash: bool,
    decode_unreserved: bool,
}

impl UriCanonicalizer {
    /// Create a canonicalizer with all normalizations enabled.
    pub fn new() -> Self {
        Self {
            normalize_case: true,
            strip_trailing_slash: true,
            decode_unreserved: true,
        }
    }

    /// Control whether scheme and authority are lowercased.
    pub fn set_normalize_case(&mut self, enabled: bool) {
        self.normalize_case = enabled;
    }

    /// Control whether trailing slashes are removed from paths.
    pub fn set_strip_trailing_slash(&mut self, enabled: bool) {
        self.strip_trailing_slash = enabled;
    }

    /// Canonicalize a URI, returning a new normalized copy.
    pub fn canonicalize(&self, uri: &ResourceUri) -> ResourceUri {
        let scheme = if self.normalize_case {
            uri.scheme.to_ascii_lowercase()
        } else {
            uri.scheme.clone()
        };
        let authority = if self.normalize_case {
            uri.authority.to_ascii_lowercase()
        } else {
            uri.authority.clone()
        };

        let mut path = uri.path.replace('\\', "/");
        // Collapse consecutive slashes into one.
        while path.contains("//") {
            path = path.replace("//", "/");
        }
        if self.decode_unreserved {
            path = decode_unreserved_chars(&path);
        }
        if self.strip_trailing_slash && path.len() > 1 && path.ends_with('/') {
            path.pop();
        }

        ResourceUri {
            scheme,
            authority,
            path,
            query: uri.query.clone(),
            fragment: uri.fragment.clone(),
        }
    }
}

impl Default for UriCanonicalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode percent-encoded characters that are in the unreserved set
/// (A-Z, a-z, 0-9, '-', '.', '_', '~') back to their literal form.
fn decode_unreserved_chars(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                let decoded = (hi << 4) | lo;
                if is_unreserved(decoded) {
                    out.push(decoded);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_unreserved(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
}

/// An LRU eviction cache for URI lookups. Stores resolved URIs up to a
/// capacity limit and evicts the least-recently-used entry when full.
pub struct UriIdentityCache {
    capacity: usize,
    /// Entries in access order (most-recently-used at the end).
    entries: Vec<(String, ResourceUri)>,
}

impl UriIdentityCache {
    /// Create a cache with the given maximum capacity. Capacity is clamped to
    /// a minimum of 1.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    /// Look up a key, promoting it to most-recently-used if found.
    pub fn get(&mut self, key: &str) -> Option<&ResourceUri> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            Some(&self.entries.last().unwrap().1)
        } else {
            None
        }
    }

    /// Insert or update a key. If the cache is at capacity, the
    /// least-recently-used entry is evicted first.
    pub fn insert(&mut self, key: impl Into<String>, uri: ResourceUri) {
        let key = key.into();
        // Remove existing entry for this key if present.
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            self.entries.remove(pos);
        }
        if self.entries.len() >= self.capacity {
            self.entries.remove(0); // evict LRU
        }
        self.entries.push((key, uri));
    }

    /// Returns the number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns true if the cache contains an entry for the given key (without
    /// changing access order).
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

/// Bulk lookup and resolution of URIs against a `UriIdentityService`.
pub struct UriIdentityBatch<'a> {
    service: &'a UriIdentityService,
}

impl<'a> UriIdentityBatch<'a> {
    /// Create a batch resolver backed by the given identity service.
    pub fn new(service: &'a UriIdentityService) -> Self {
        Self { service }
    }

    /// Resolve a slice of keys, returning results in the same order.
    /// Unresolvable keys map to `None`.
    pub fn resolve_all(&self, keys: &[&str]) -> Vec<Option<ResourceUri>> {
        keys.iter()
            .map(|k| self.service.resolve_or_parse(k))
            .collect()
    }

    /// Resolve only keys that match a given predicate.
    pub fn resolve_filtered<F>(&self, keys: &[&str], predicate: F) -> Vec<(String, ResourceUri)>
    where
        F: Fn(&str) -> bool,
    {
        keys.iter()
            .filter(|k| predicate(k))
            .filter_map(|k| {
                self.service
                    .resolve_or_parse(k)
                    .map(|uri| (k.to_string(), uri))
            })
            .collect()
    }

    /// Count how many of the given keys successfully resolve.
    pub fn count_resolvable(&self, keys: &[&str]) -> usize {
        keys.iter()
            .filter(|k| self.service.resolve_or_parse(k).is_some())
            .count()
    }
}

/// Compare two URIs with case-insensitive scheme and authority but
/// case-sensitive path, query, and fragment.
pub fn uri_compare_case_insensitive(a: &ResourceUri, b: &ResourceUri) -> bool {
    a.scheme.eq_ignore_ascii_case(&b.scheme)
        && a.authority.eq_ignore_ascii_case(&b.authority)
        && a.path == b.path
        && a.query == b.query
        && a.fragment == b.fragment
}


// === URI Identity Resolver Cache ===

/// URI Identity Resolver Cache implementation.
#[derive(Debug, Clone)]
pub struct UriIdentityResolverCache {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: UriIdentityResolverCacheStats,
}

/// Statistics for UriIdentityResolverCache.
#[derive(Debug, Clone, Default)]
pub struct UriIdentityResolverCacheStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl UriIdentityResolverCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl UriIdentityResolverCache {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: UriIdentityResolverCacheStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &UriIdentityResolverCacheStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for UriIdentityResolverCache {
    fn default() -> Self {
        Self::new()
    }
}

// === URI Identity Batch Comparator ===

/// Priority level for UriIdentityBatchComparator items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UriIdentityBatchComparatorPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl UriIdentityBatchComparatorPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for UriIdentityBatchComparatorPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// URI Identity Batch Comparator implementation.
#[derive(Debug, Clone)]
pub struct UriIdentityBatchComparator {
    items: Vec<UriIdentityBatchComparatorItem>,
    max_items: usize,
    default_priority: UriIdentityBatchComparatorPriority,
}

/// A single item in UriIdentityBatchComparator.
#[derive(Debug, Clone)]
pub struct UriIdentityBatchComparatorItem {
    pub id: String,
    pub label: String,
    pub priority: UriIdentityBatchComparatorPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl UriIdentityBatchComparatorItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: UriIdentityBatchComparatorPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: UriIdentityBatchComparatorPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl UriIdentityBatchComparator {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: UriIdentityBatchComparatorPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: UriIdentityBatchComparatorItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<UriIdentityBatchComparatorItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&UriIdentityBatchComparatorItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: UriIdentityBatchComparatorPriority) -> Vec<&UriIdentityBatchComparatorItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&UriIdentityBatchComparatorItem> {
        let mut sorted: Vec<&UriIdentityBatchComparatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&UriIdentityBatchComparatorItem> {
        let mut sorted: Vec<&UriIdentityBatchComparatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&UriIdentityBatchComparatorItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: UriIdentityBatchComparatorPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> UriIdentityBatchComparatorPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &UriIdentityBatchComparatorItem> {
        self.items.iter()
    }
}

impl Default for UriIdentityBatchComparator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// uriidentity – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XUriidentityValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XUriidentityValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(m) | Self::Warning(m) => Some(m),
        }
    }
}

/// A key-value pair with optional metadata tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XUriidentityTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XUriidentityTaggedEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

/// Validate that a string is non-empty and within a max length.
pub fn x_uriidentity_validate_string(value: &str, max_len: usize) -> XUriidentityValidationResult {
    if value.is_empty() {
        return XUriidentityValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XUriidentityValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XUriidentityValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_uriidentity_validate_range(value: i64, min: i64, max: i64) -> XUriidentityValidationResult {
    if value < min || value > max {
        XUriidentityValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XUriidentityValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_uriidentity_filter_by_tag<'a>(
    entries: &'a [XUriidentityTaggedEntry],
    tag: &str,
) -> Vec<&'a XUriidentityTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_uriidentity_group_by_tag(
    entries: &[XUriidentityTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XUriidentityTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_uriidentity_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_uriidentity_dedup_entries(entries: Vec<XUriidentityTaggedEntry>) -> Vec<XUriidentityTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
}



// ---------------------------------------------------------------------------
// uriidentity – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for URI identity and comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YUriidentityUriComparison {
    Equal,
    CaseInsensitiveEqual,
    Different,
    SameAuthority,
}

impl YUriidentityUriComparison {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Equal => 0,
            Self::CaseInsensitiveEqual => 1,
            Self::Different => 2,
            Self::SameAuthority => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Equal => "Equal",
            Self::CaseInsensitiveEqual => "CaseInsensitiveEqual",
            Self::Different => "Different",
            Self::SameAuthority => "SameAuthority",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YUriidentityUriComparison] {
        &[
            YUriidentityUriComparison::Equal,
            YUriidentityUriComparison::CaseInsensitiveEqual,
            YUriidentityUriComparison::Different,
            YUriidentityUriComparison::SameAuthority,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YUriidentityUriComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks URI normalizer data.
#[derive(Debug, Clone)]
pub struct YUriidentityUriNormalizer {
    pub scheme_map: Vec<(String, String)>,
    pub case_sensitive: bool,
    pub strip_trailing_slash: bool,
}

impl YUriidentityUriNormalizer {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            scheme_map: Vec::new(),
            case_sensitive: false,
            strip_trailing_slash: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.scheme_map.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.scheme_map.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.scheme_map.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YUriidentityUriNormalizer({}: {:?})", "scheme_map", self.scheme_map)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_uriidentity_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_uriidentity_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_uriidentity_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_uriidentity_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_uriidentity_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_uriidentity_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_uriidentity_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_uriidentity_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// uriidentity – Extended URI canonicalizer helpers
// ---------------------------------------------------------------------------

/// Priority levels for URI canonicalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZUriidentityPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZUriidentityPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZUriidentityPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZUriidentityPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks URI canonicalizer data.
#[derive(Debug, Clone)]
pub struct ZUriidentityUriCanonicalizer {
    pub rewrites: Vec<(String, String)>,
    pub normalize_case: bool,
    pub strip_fragment: bool,
}

impl ZUriidentityUriCanonicalizer {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            rewrites: Vec::new(),
            normalize_case: false,
            strip_fragment: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.rewrites.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.rewrites.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.rewrites.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZUriidentityUriCanonicalizer[normalize_case={:?}, strip_fragment={:?}]", self.normalize_case, self.strip_fragment)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.strip_fragment = !c.strip_fragment;
        c
    }
}

/// Compute a simple rolling hash for URI canonicalizer.
pub fn z_uriidentity_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_uriidentity_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_uriidentity_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_uriidentity_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_uriidentity_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_uriidentity_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_uriidentity_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 97
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer97 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer97 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_97(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_97<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_97<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_97(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_97(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_parts() {
        let uri = ResourceUri::file("/home/user/file.rs");
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.filename(), Some("file.rs"));
        assert_eq!(uri.extension(), Some("rs"));
    }

    #[test]
    fn parse_and_roundtrip() {
        let uri = ResourceUri::from_string("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com");
        assert_eq!(uri.path, "/path");
        assert_eq!(uri.query.as_deref(), Some("q=1"));
        assert_eq!(uri.fragment.as_deref(), Some("frag"));
        assert_eq!(uri.to_string(), "https://example.com/path?q=1#frag");
    }

    #[test]
    fn case_insensitive_equality() {
        let a = ResourceUri::file("/Home/User/File.rs");
        let b = ResourceUri::file("/home/user/file.rs");
        assert!(are_equal(&a, &b));
    }

    #[test]
    fn identity_service() {
        let mut svc = UriIdentityService::new();
        svc.register("main", ResourceUri::file("/src/main.rs"));
        assert!(svc.resolve("main").is_some());
        assert!(svc.resolve("other").is_none());
    }

    #[test]
    fn try_parse_valid() {
        let uri = ResourceUri::try_parse("https://host.com/path").unwrap();
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "host.com");
        assert_eq!(uri.path, "/path");
    }

    #[test]
    fn try_parse_missing_scheme() {
        let err = ResourceUri::try_parse("no-scheme-here").unwrap_err();
        assert_eq!(err, UriError::MissingScheme);
    }

    #[test]
    fn try_parse_empty_scheme() {
        let err = ResourceUri::try_parse("://oops").unwrap_err();
        assert_eq!(err, UriError::MissingScheme);
    }

    #[test]
    fn try_parse_empty_file_path() {
        let err = ResourceUri::try_parse("file://").unwrap_err();
        assert_eq!(err, UriError::EmptyPath);
    }

    #[test]
    fn parent_uri() {
        let uri = ResourceUri::file("/home/user/project/file.rs");
        let parent = uri.parent().unwrap();
        assert_eq!(parent.path, "/home/user/project");
        let grandparent = parent.parent().unwrap();
        assert_eq!(grandparent.path, "/home/user");
    }

    #[test]
    fn parent_root_returns_none() {
        let uri = ResourceUri::file("/");
        assert!(uri.parent().is_none());
    }

    #[test]
    fn builder_fragment_and_query() {
        let uri = ResourceUri::file("/index.html")
            .with_query("page=1")
            .with_fragment("top");
        assert_eq!(uri.query.as_deref(), Some("page=1"));
        assert_eq!(uri.fragment.as_deref(), Some("top"));
        assert_eq!(uri.to_string(), "file:///index.html?page=1#top");
    }

    #[test]
    fn is_file_check() {
        assert!(ResourceUri::file("/tmp").is_file());
        assert!(!ResourceUri::new("https", "/path").is_file());
    }

    #[test]
    fn display_impl() {
        let uri = ResourceUri::file("/a/b.txt");
        let displayed = format!("{}", uri);
        assert_eq!(displayed, "file:///a/b.txt");
    }

    #[test]
    fn service_unregister() {
        let mut svc = UriIdentityService::new();
        svc.register("key", ResourceUri::file("/tmp"));
        assert_eq!(svc.len(), 1);
        let removed = svc.unregister("key");
        assert!(removed.is_some());
        assert!(svc.is_empty());
    }

    #[test]
    fn service_resolve_or_parse() {
        let mut svc = UriIdentityService::new();
        svc.register("alias", ResourceUri::file("/registered"));

        let resolved = svc.resolve_or_parse("alias").unwrap();
        assert_eq!(resolved.path, "/registered");

        let parsed = svc.resolve_or_parse("https://example.com/fallback").unwrap();
        assert_eq!(parsed.scheme, "https");
        assert_eq!(parsed.path, "/fallback");

        assert!(svc.resolve_or_parse("not-a-uri").is_none());
    }

    #[test]
    fn service_len_empty_clear() {
        let mut svc = UriIdentityService::new();
        assert!(svc.is_empty());
        svc.register("a", ResourceUri::file("/a"));
        svc.register("b", ResourceUri::file("/b"));
        assert_eq!(svc.len(), 2);
        svc.clear();
        assert!(svc.is_empty());
    }

    #[test]
    fn percent_encode_decode_roundtrip() {
        let original = "/path/to/my file (1).txt";
        let encoded = percent_encode(original);
        assert!(encoded.contains("%20"));
        assert!(encoded.contains("%28"));
        let decoded = percent_decode(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn error_display() {
        assert_eq!(UriError::InvalidUri.to_string(), "invalid URI");
        assert_eq!(UriError::MissingScheme.to_string(), "missing scheme");
        assert_eq!(UriError::EmptyPath.to_string(), "empty path");
    }

    #[test]
    fn eq_urierror_same() {
        assert_eq!(UriError::InvalidUri, UriError::InvalidUri);
    }

    #[test]
    fn ne_urierror_diff() {
        assert_ne!(UriError::InvalidUri, UriError::MissingScheme);
    }

    #[test]
    fn display_urierror_variants() {
        assert!(!UriError::InvalidUri.to_string().is_empty());
        assert!(!UriError::MissingScheme.to_string().is_empty());
        assert!(!UriError::EmptyPath.to_string().is_empty());
    }

    #[test]
    fn normalizer_unix_case_sensitive() {
        let norm = UriIdentityNormalizer::unix();
        let a = ResourceUri::file("/home/user/File.rs");
        let b = ResourceUri::file("/home/user/file.rs");
        assert!(!norm.are_equal(&a, &b));
    }

    #[test]
    fn normalizer_windows_case_insensitive() {
        let norm = UriIdentityNormalizer::windows();
        let a = ResourceUri::file("/C:/Users/File.rs");
        let b = ResourceUri::file("/C:/users/file.rs");
        assert!(norm.are_equal(&a, &b));
    }

    #[test]
    fn normalizer_strips_trailing_slash() {
        let norm = UriIdentityNormalizer::new();
        assert_eq!(norm.normalize_path("/home/user/"), "/home/user");
    }

    #[test]
    fn normalizer_preserves_root_slash() {
        let norm = UriIdentityNormalizer::new();
        assert_eq!(norm.normalize_path("/"), "/");
    }

    #[test]
    fn normalizer_normalizes_separators() {
        let norm = UriIdentityNormalizer::new();
        assert_eq!(norm.normalize_path("C:\\Users\\file.rs"), "C:/Users/file.rs");
    }

    #[test]
    fn canonical_form_basic() {
        let uri = ResourceUri::new("FILE", "/Home/User/file.rs");
        let canonical = uri_canonical_form(&uri);
        assert_eq!(canonical, "file:///Home/User/file.rs");
    }

    #[test]
    fn canonical_form_strips_trailing_slash() {
        let uri = ResourceUri::new("file", "/home/user/");
        let canonical = uri_canonical_form(&uri);
        assert_eq!(canonical, "file:///home/user");
    }

    #[test]
    fn canonical_form_with_query_fragment() {
        let uri = ResourceUri::new("https", "/path")
            .with_query("key=val")
            .with_fragment("section");
        let canonical = uri_canonical_form(&uri);
        assert!(canonical.ends_with("?key=val#section"));
    }

    #[test]
    fn canonical_form_normalizes_percent_encoding() {
        let uri = ResourceUri::new("file", "/path%2fto%2Ffile");
        let canonical = uri_canonical_form(&uri);
        assert!(canonical.contains("%2F"));
        assert!(!canonical.contains("%2f"));
    }

    #[test]
    fn relative_path_same_dir() {
        let base = ResourceUri::file("/home/user");
        let target = ResourceUri::file("/home/user");
        assert_eq!(uri_relative_path(&base, &target), Some(".".to_string()));
    }

    #[test]
    fn relative_path_child() {
        let base = ResourceUri::file("/home/user");
        let target = ResourceUri::file("/home/user/file.rs");
        assert_eq!(uri_relative_path(&base, &target), Some("file.rs".to_string()));
    }

    #[test]
    fn relative_path_sibling() {
        let base = ResourceUri::file("/home/user/a");
        let target = ResourceUri::file("/home/user/b");
        assert_eq!(uri_relative_path(&base, &target), Some("../b".to_string()));
    }

    #[test]
    fn relative_path_parent() {
        let base = ResourceUri::file("/home/user/sub");
        let target = ResourceUri::file("/home/user");
        assert_eq!(uri_relative_path(&base, &target), Some("..".to_string()));
    }

    #[test]
    fn relative_path_different_scheme() {
        let base = ResourceUri::new("file", "/path");
        let target = ResourceUri::new("https", "/path");
        assert!(uri_relative_path(&base, &target).is_none());
    }

    #[test]
    fn relative_path_deep() {
        let base = ResourceUri::file("/a/b/c/d");
        let target = ResourceUri::file("/a/x/y");
        assert_eq!(uri_relative_path(&base, &target), Some("../../../x/y".to_string()));
    }

    #[test]
    fn normalizer_default_settings() {
        let norm = UriIdentityNormalizer::default();
        assert!(!norm.ignore_path_case);
        assert!(norm.normalize_separators);
        assert!(norm.strip_trailing_slash);
    }

    #[test]
    fn normalizer_equal_with_backslash_normalization() {
        let norm = UriIdentityNormalizer::new();
        let a = ResourceUri::file("/home/user/file.rs");
        let mut b = ResourceUri::file("/home/user/file.rs");
        b.path = "\\home\\user\\file.rs".to_string();
        assert!(norm.are_equal(&a, &b));
    }

    #[test]
    fn normalize_percent_encoding_mixed_case() {
        let result = normalize_percent_encoding("/path%2fto%2Ffile%3a");
        assert_eq!(result, "/path%2Fto%2Ffile%3A");
    }

    #[test]
    fn uriidentity_stats_new_defaults() {
        let stats = UriidentityStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn uriidentity_stats_record_success() {
        let mut stats = UriidentityStats::new();
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
    fn uriidentity_stats_record_failure() {
        let mut stats = UriidentityStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn uriidentity_stats_reset() {
        let mut stats = UriidentityStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn uriidentity_stats_merge() {
        let mut a = UriidentityStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = UriidentityStats::new();
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
    fn uriidentity_stats_display() {
        let mut stats = UriidentityStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn uriidentity_stats_default() {
        let stats = UriidentityStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn uriidentity_validator_accepts_valid_name() {
        let v = UriidentityValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn uriidentity_validator_rejects_empty() {
        let v = UriidentityValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn uriidentity_validator_rejects_too_long() {
        let v = UriidentityValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn uriidentity_validator_forbidden_prefix() {
        let v = UriidentityValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn uriidentity_validator_allowed_chars() {
        let v = UriidentityValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn uriidentity_validator_range() {
        let v = UriidentityValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn uriidentity_sanitize_removes_control() {
        let result = UriidentityValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn uriidentity_truncate_short_string() {
        assert_eq!(UriidentityValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn uriidentity_truncate_long_string() {
        let result = UriidentityValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn uriidentity_is_ascii_printable() {
        assert!(UriidentityValidator::is_ascii_printable("Hello World 123"));
        assert!(!UriidentityValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn test_uri_identity_hash_equal() {
        let a = ResourceUri::file("/home/user/file.rs");
        let b = ResourceUri::file("/home/user/file.rs");
        assert_eq!(uri_identity_hash(&a), uri_identity_hash(&b));
    }

    #[test]
    fn test_uri_identity_hash_different() {
        let a = ResourceUri::file("/home/user/file.rs");
        let b = ResourceUri::file("/home/user/other.rs");
        assert_ne!(uri_identity_hash(&a), uri_identity_hash(&b));
    }

    #[test]
    fn test_uri_same_document_true() {
        let a = ResourceUri::from_string("https://example.com/path?q=1#frag").unwrap();
        let b = ResourceUri::from_string("https://example.com/path?q=2#other").unwrap();
        assert!(uri_same_document(&a, &b));
    }

    #[test]
    fn test_uri_same_document_false_path() {
        let a = ResourceUri::from_string("https://example.com/path1").unwrap();
        let b = ResourceUri::from_string("https://example.com/path2").unwrap();
        assert!(!uri_same_document(&a, &b));
    }

    #[test]
    fn test_uri_same_document_ignores_fragment() {
        let mut a = ResourceUri::file("/home/user/file.rs");
        a.fragment = Some("line10".to_string());
        let mut b = ResourceUri::file("/home/user/file.rs");
        b.fragment = Some("line20".to_string());
        assert!(uri_same_document(&a, &b));
    }

    #[test]
    fn test_uri_components_parse() {
        let c = UriComponents::parse("https://example.com/path?q=1#frag");
        assert_eq!(c.scheme.as_deref(), Some("https"));
        assert_eq!(c.authority.as_deref(), Some("example.com"));
        assert_eq!(c.path, "/path");
        assert_eq!(c.query.as_deref(), Some("q=1"));
        assert_eq!(c.fragment.as_deref(), Some("frag"));
        assert!(c.has_authority());
        assert!(c.has_query());
        assert!(c.has_fragment());
    }

    #[test]
    fn test_uri_components_to_resource_uri() {
        let c = UriComponents::parse("https://example.com/path?q=1#frag");
        let uri = c.to_resource_uri();
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com");
        assert_eq!(uri.path, "/path");
        assert_eq!(uri.query.as_deref(), Some("q=1"));
        assert_eq!(uri.fragment.as_deref(), Some("frag"));
    }

    // ---- UriPattern tests ----

    #[test]
    fn uri_pattern_exact_path() {
        let pattern = UriPattern::path_only("/src/main.rs");
        let uri = ResourceUri::file("/src/main.rs");
        assert!(pattern.matches(&uri));

        let uri2 = ResourceUri::file("/src/lib.rs");
        assert!(!pattern.matches(&uri2));
    }

    #[test]
    fn uri_pattern_wildcard() {
        let pattern = UriPattern::path_only("/src/*.rs");
        let uri1 = ResourceUri::file("/src/main.rs");
        let uri2 = ResourceUri::file("/src/lib.rs");
        let uri3 = ResourceUri::file("/src/deep/nested.rs");
        assert!(pattern.matches(&uri1));
        assert!(pattern.matches(&uri2));
        assert!(!pattern.matches(&uri3));
    }

    #[test]
    fn uri_pattern_double_wildcard() {
        let pattern = UriPattern::path_only("/**/*.rs");
        let uri1 = ResourceUri::file("/src/main.rs");
        let uri2 = ResourceUri::file("/src/deep/nested.rs");
        assert!(pattern.matches(&uri1));
        assert!(pattern.matches(&uri2));
    }

    #[test]
    fn uri_pattern_with_scheme() {
        let pattern = UriPattern::with_scheme("https", "/api/*");
        let uri1 = ResourceUri::new("https", "/api/users");
        let uri2 = ResourceUri::new("http", "/api/users");
        assert!(pattern.matches(&uri1));
        assert!(!pattern.matches(&uri2));
    }

    // ---- URI normalization tests ----

    #[test]
    fn normalize_uri_removes_trailing_slash() {
        let uri = ResourceUri::file("/home/user/project/");
        let normalized = normalize_uri(&uri);
        assert_eq!(normalized.path, "/home/user/project");
    }

    #[test]
    fn normalize_uri_fixes_backslashes() {
        let uri = ResourceUri::new("file", "C:\\Users\\test\\file.txt");
        let normalized = normalize_uri(&uri);
        assert_eq!(normalized.path, "C:/Users/test/file.txt");
    }

    #[test]
    fn uri_join_relative() {
        let base = ResourceUri::file("/home/user/project/src/main.rs");
        let joined = uri_join(&base, "../lib.rs");
        assert_eq!(joined.path, "/home/user/project/lib.rs");
    }

    #[test]
    fn uri_is_child_check() {
        let parent = ResourceUri::file("/home/user");
        let child = ResourceUri::file("/home/user/project/file.rs");
        let non_child = ResourceUri::file("/home/other/file.rs");
        assert!(uri_is_child(&parent, &child));
        assert!(!uri_is_child(&parent, &non_child));
    }

    #[test]
    fn uri_depth_computation() {
        let uri = ResourceUri::file("/home/user/project/src");
        assert_eq!(uri_depth(&uri), 4);
        let root = ResourceUri::file("/");
        assert_eq!(uri_depth(&root), 0);
    }

    #[test]
    fn uri_unique_schemes_deduplicates() {
        let uris = vec![
            ResourceUri::file("/a"),
            ResourceUri::new("https", "/b"),
            ResourceUri::file("/c"),
        ];
        let schemes = uri_unique_schemes(&uris);
        assert_eq!(schemes, vec!["file", "https"]);
    }

    #[test]
    fn uri_unique_schemes_empty() {
        assert!(uri_unique_schemes(&[]).is_empty());
    }

    #[test]
    fn uri_group_by_scheme_groups() {
        let uris = vec![
            ResourceUri::file("/a"),
            ResourceUri::new("https", "/b"),
            ResourceUri::file("/c"),
        ];
        let groups = uri_group_by_scheme(&uris);
        assert_eq!(groups.get("file").unwrap().len(), 2);
        assert_eq!(groups.get("https").unwrap().len(), 1);
    }

    #[test]
    fn uri_sort_by_depth_orders() {
        let mut uris = vec![
            ResourceUri::file("/a/b/c"),
            ResourceUri::file("/a"),
            ResourceUri::file("/a/b"),
        ];
        uri_sort_by_depth(&mut uris);
        assert_eq!(uris[0].path, "/a");
        assert_eq!(uris[1].path, "/a/b");
        assert_eq!(uris[2].path, "/a/b/c");
    }

    #[test]
    fn uri_find_children_filters() {
        let parent = ResourceUri::file("/home/user");
        let uris = vec![
            ResourceUri::file("/home/user/file.rs"),
            ResourceUri::file("/home/other/file.rs"),
            ResourceUri::file("/home/user/sub/deep.rs"),
        ];
        let children = uri_find_children(&parent, &uris);
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn uri_common_path_finds_prefix() {
        let uris = vec![
            ResourceUri::file("/home/user/project/src/a.rs"),
            ResourceUri::file("/home/user/project/src/b.rs"),
        ];
        assert_eq!(uri_common_path(&uris), "/home/user/project/src/");
    }

    #[test]
    fn uri_common_path_empty() {
        assert_eq!(uri_common_path(&[]), "");
    }

    #[test]
    fn uri_same_origin_true() {
        let a = ResourceUri::file("/a");
        let b = ResourceUri::file("/b");
        assert!(uri_same_origin(&a, &b));
    }

    #[test]
    fn uri_same_origin_false() {
        let a = ResourceUri::file("/a");
        let b = ResourceUri::new("https", "/a");
        assert!(!uri_same_origin(&a, &b));
    }

    #[test]
    fn uri_has_extension_checks() {
        let uri = ResourceUri::file("/src/main.rs");
        assert!(uri_has_extension(&uri, "rs"));
        assert!(uri_has_extension(&uri, "RS"));
        assert!(!uri_has_extension(&uri, "py"));
    }

    #[test]
    fn uri_has_extension_no_ext() {
        let uri = ResourceUri::file("/Makefile");
        assert!(!uri_has_extension(&uri, "rs"));
    }

    #[test]
    fn uri_dirname_basic() {
        let uri = ResourceUri::file("/home/user/file.rs");
        assert_eq!(uri_dirname(&uri), "/home/user");
    }

    #[test]
    fn uri_dirname_root_file() {
        let uri = ResourceUri::file("/file.rs");
        assert_eq!(uri_dirname(&uri), "/");
    }

    #[test]
    fn uri_basename_basic() {
        let uri = ResourceUri::file("/home/user/file.rs");
        assert_eq!(uri_basename(&uri), "file.rs");
    }

    #[test]
    fn uri_with_extension_replaces() {
        let uri = ResourceUri::file("/src/main.rs");
        let changed = uri_with_extension(&uri, "ts");
        assert_eq!(changed.path, "/src/main.ts");
    }

    #[test]
    fn uri_with_extension_adds_when_missing() {
        let uri = ResourceUri::file("/Makefile");
        let changed = uri_with_extension(&uri, "bak");
        assert_eq!(changed.path, "/Makefile.bak");
    }

    #[test]
    fn uri_append_path_basic() {
        let uri = ResourceUri::file("/home/user");
        let appended = uri_append_path(&uri, "docs/readme.md");
        assert_eq!(appended.path, "/home/user/docs/readme.md");
    }

    #[test]
    fn uri_segment_count_works() {
        let uri = ResourceUri::file("/a/b/c/d");
        assert_eq!(uri_segment_count(&uri), 4);
    }

    #[test]
    fn uri_is_root_single_segment() {
        let uri = ResourceUri::file("/file");
        assert!(uri_is_root(&uri));
    }

    #[test]
    fn uri_is_root_deep_path() {
        let uri = ResourceUri::file("/a/b");
        assert!(!uri_is_root(&uri));
    }

    #[test]
    fn uri_path_segments_splits() {
        let uri = ResourceUri::file("/src/lib.rs");
        assert_eq!(uri_path_segments(&uri), vec!["src", "lib.rs"]);
    }

    #[test]
    fn uri_with_query_sets_params() {
        let uri = ResourceUri::file("/path");
        let with_q = uri_with_query(&uri, &[("key", "val"), ("a", "b")]);
        assert_eq!(with_q.query, Some("key=val&a=b".to_string()));
    }

    #[test]
    fn uri_with_fragment_sets() {
        let uri = ResourceUri::file("/path");
        let with_f = uri_with_fragment(&uri, "section-1");
        assert_eq!(with_f.fragment, Some("section-1".to_string()));
    }

    #[test]
    fn uri_strip_query_fragment_cleans() {
        let mut uri = ResourceUri::file("/path");
        uri.query = Some("x=1".into());
        uri.fragment = Some("top".into());
        let clean = uri_strip_query_fragment(&uri);
        assert!(clean.query.is_none());
        assert!(clean.fragment.is_none());
        assert_eq!(clean.path, "/path");
    }

    // --- UriCanonicalizer tests ---

    #[test]
    fn canonicalizer_lowercases_scheme_and_authority() {
        let c = UriCanonicalizer::new();
        let uri = ResourceUri {
            scheme: "HTTP".into(),
            authority: "Example.COM".into(),
            path: "/Path".into(),
            query: None,
            fragment: None,
        };
        let norm = c.canonicalize(&uri);
        assert_eq!(norm.scheme, "http");
        assert_eq!(norm.authority, "example.com");
        assert_eq!(norm.path, "/Path"); // path case preserved
    }

    #[test]
    fn canonicalizer_strips_trailing_slash() {
        let c = UriCanonicalizer::new();
        let uri = ResourceUri::new("file", "/home/user/");
        let norm = c.canonicalize(&uri);
        assert_eq!(norm.path, "/home/user");
    }

    #[test]
    fn canonicalizer_preserves_root_slash() {
        let c = UriCanonicalizer::new();
        let uri = ResourceUri::new("file", "/");
        let norm = c.canonicalize(&uri);
        assert_eq!(norm.path, "/");
    }

    #[test]
    fn canonicalizer_normalizes_backslashes() {
        let c = UriCanonicalizer::new();
        let uri = ResourceUri::new("file", "\\home\\user\\file.rs");
        let norm = c.canonicalize(&uri);
        assert_eq!(norm.path, "/home/user/file.rs");
    }

    #[test]
    fn canonicalizer_collapses_double_slashes() {
        let c = UriCanonicalizer::new();
        let uri = ResourceUri::new("file", "/home//user///file.rs");
        let norm = c.canonicalize(&uri);
        assert_eq!(norm.path, "/home/user/file.rs");
    }

    #[test]
    fn canonicalizer_decodes_unreserved_percent() {
        let c = UriCanonicalizer::new();
        // %61 = 'a', %7E = '~'
        let uri = ResourceUri::new("file", "/p%61th/%7Efile");
        let norm = c.canonicalize(&uri);
        assert_eq!(norm.path, "/path/~file");
    }

    #[test]
    fn canonicalizer_keeps_reserved_percent() {
        let c = UriCanonicalizer::new();
        // %2F = '/' (reserved), should stay encoded
        let uri = ResourceUri::new("file", "/path%2Fencoded");
        let norm = c.canonicalize(&uri);
        assert_eq!(norm.path, "/path%2Fencoded");
    }

    // --- UriIdentityCache tests ---

    #[test]
    fn cache_insert_and_get() {
        let mut cache = UriIdentityCache::new(4);
        cache.insert("a", ResourceUri::file("/a"));
        assert!(cache.get("a").is_some());
        assert_eq!(cache.get("a").unwrap().path, "/a");
        assert!(cache.get("missing").is_none());
    }

    #[test]
    fn cache_evicts_lru() {
        let mut cache = UriIdentityCache::new(2);
        cache.insert("a", ResourceUri::file("/a"));
        cache.insert("b", ResourceUri::file("/b"));
        // "a" is LRU
        cache.insert("c", ResourceUri::file("/c"));
        assert!(cache.get("a").is_none(), "a should have been evicted");
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn cache_get_promotes_to_mru() {
        let mut cache = UriIdentityCache::new(2);
        cache.insert("a", ResourceUri::file("/a"));
        cache.insert("b", ResourceUri::file("/b"));
        // access "a" to promote it; now "b" is LRU
        let _ = cache.get("a");
        cache.insert("c", ResourceUri::file("/c"));
        assert!(cache.get("b").is_none(), "b should have been evicted");
        assert!(cache.get("a").is_some());
    }

    // --- UriIdentityBatch tests ---

    #[test]
    fn batch_resolve_all() {
        let mut svc = UriIdentityService::new();
        svc.register("editor", ResourceUri::file("/editor"));
        let batch = UriIdentityBatch::new(&svc);
        let results = batch.resolve_all(&["editor", "missing"]);
        assert!(results[0].is_some());
        assert!(results[1].is_none());
    }

    #[test]
    fn batch_count_resolvable() {
        let mut svc = UriIdentityService::new();
        svc.register("x", ResourceUri::file("/x"));
        svc.register("y", ResourceUri::file("/y"));
        let batch = UriIdentityBatch::new(&svc);
        assert_eq!(batch.count_resolvable(&["x", "y", "z"]), 2);
    }

    // --- uri_compare_case_insensitive tests ---

    #[test]
    fn case_insensitive_compare_scheme_authority() {
        let a = ResourceUri {
            scheme: "HTTP".into(),
            authority: "Example.COM".into(),
            path: "/path".into(),
            query: None,
            fragment: None,
        };
        let b = ResourceUri {
            scheme: "http".into(),
            authority: "example.com".into(),
            path: "/path".into(),
            query: None,
            fragment: None,
        };
        assert!(uri_compare_case_insensitive(&a, &b));
    }

    #[test]
    fn case_insensitive_compare_path_sensitive() {
        let a = ResourceUri::new("http", "/Path");
        let b = ResourceUri::new("http", "/path");
        assert!(!uri_compare_case_insensitive(&a, &b));
    }

    #[test]
    fn uriIdentityResolverCache_new() {
        let s = UriIdentityResolverCache::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn uriIdentityResolverCache_add_contains() {
        let mut s = UriIdentityResolverCache::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn uriIdentityResolverCache_add_duplicate() {
        let mut s = UriIdentityResolverCache::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn uriIdentityResolverCache_remove() {
        let mut s = UriIdentityResolverCache::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn uriIdentityResolverCache_capacity() {
        let s = UriIdentityResolverCache::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn uriIdentityResolverCache_search() {
        let mut s = UriIdentityResolverCache::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn uriIdentityResolverCache_stats() {
        let mut s = UriIdentityResolverCache::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn uriIdentityBatchComparator_new() {
        let m = UriIdentityBatchComparator::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn uriIdentityBatchComparator_add_find() {
        let mut m = UriIdentityBatchComparator::new();
        m.add(UriIdentityBatchComparatorItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn uriIdentityBatchComparator_priority_filter() {
        let mut m = UriIdentityBatchComparator::new();
        m.add(UriIdentityBatchComparatorItem::new("a", "A").with_priority(UriIdentityBatchComparatorPriority::High));
        m.add(UriIdentityBatchComparatorItem::new("b", "B").with_priority(UriIdentityBatchComparatorPriority::Low));
        m.add(UriIdentityBatchComparatorItem::new("c", "C").with_priority(UriIdentityBatchComparatorPriority::High));
        assert_eq!(m.by_priority(UriIdentityBatchComparatorPriority::High).len(), 2);
    }

    #[test]
    fn uriIdentityBatchComparator_remove() {
        let mut m = UriIdentityBatchComparator::new();
        m.add(UriIdentityBatchComparatorItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn uriIdentityBatchComparator_search() {
        let mut m = UriIdentityBatchComparator::new();
        m.add(UriIdentityBatchComparatorItem::new("id1", "Hello World"));
        m.add(UriIdentityBatchComparatorItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn uriIdentityBatchComparator_total_weight() {
        let mut m = UriIdentityBatchComparator::new();
        m.add(UriIdentityBatchComparatorItem::new("a", "A").with_priority(UriIdentityBatchComparatorPriority::Critical));
        m.add(UriIdentityBatchComparatorItem::new("b", "B").with_priority(UriIdentityBatchComparatorPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn uriIdentityBatchComparator_capacity_limit() {
        let mut m = UriIdentityBatchComparator::new().with_max_items(2);
        m.add(UriIdentityBatchComparatorItem::new("1", "one"));
        m.add(UriIdentityBatchComparatorItem::new("2", "two"));
        assert!(!m.add(UriIdentityBatchComparatorItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn uriIdentityBatchComparator_sorted_by_priority() {
        let mut m = UriIdentityBatchComparator::new();
        m.add(UriIdentityBatchComparatorItem::new("lo", "Low").with_priority(UriIdentityBatchComparatorPriority::Low));
        m.add(UriIdentityBatchComparatorItem::new("hi", "High").with_priority(UriIdentityBatchComparatorPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn uriIdentityBatchComparator_item_metadata() {
        let mut item = UriIdentityBatchComparatorItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn uriIdentityResolverCache_enabled_toggle() {
        let mut s = UriIdentityResolverCache::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn uriIdentityBatchComparator_priority_display() {
        assert_eq!(format!("{}", UriIdentityBatchComparatorPriority::High), "high");
        assert_eq!(format!("{}", UriIdentityBatchComparatorPriority::Low), "low");
    }


    // -- uriidentity additional tests -------------------------------------------

    #[test]
    fn x_uriidentity_validation_ok() {
        let r = x_uriidentity_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_uriidentity_validation_empty() {
        let r = x_uriidentity_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_uriidentity_validation_too_long() {
        let r = x_uriidentity_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_uriidentity_validate_range_ok() {
        assert!(x_uriidentity_validate_range(5, 1, 10).is_ok());
        assert!(x_uriidentity_validate_range(1, 1, 10).is_ok());
        assert!(x_uriidentity_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_uriidentity_validate_range_out() {
        assert!(!x_uriidentity_validate_range(0, 1, 10).is_ok());
        assert!(!x_uriidentity_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_uriidentity_tagged_entry_basic() {
        let e = XUriidentityTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_uriidentity_tagged_entry_with_tag() {
        let e = XUriidentityTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_uriidentity_filter_by_tag_basic() {
        let entries = vec![
            XUriidentityTaggedEntry::new("a", "1").with_tag("x"),
            XUriidentityTaggedEntry::new("b", "2").with_tag("y"),
            XUriidentityTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_uriidentity_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_uriidentity_group_by_tag_basic() {
        let entries = vec![
            XUriidentityTaggedEntry::new("a", "1").with_tag("x"),
            XUriidentityTaggedEntry::new("b", "2"),
            XUriidentityTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_uriidentity_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_uriidentity_djb2_hash_deterministic() {
        let h1 = x_uriidentity_djb2_hash("hello");
        let h2 = x_uriidentity_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_uriidentity_djb2_hash("hello"), x_uriidentity_djb2_hash("world"));
    }

    #[test]
    fn x_uriidentity_dedup_entries_basic() {
        let entries = vec![
            XUriidentityTaggedEntry::new("a", "1"),
            XUriidentityTaggedEntry::new("a", "2"),
            XUriidentityTaggedEntry::new("b", "3"),
        ];
        let deduped = x_uriidentity_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_uriidentity_validation_result_warning() {
        let w = XUriidentityValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_uriidentity_filter_by_tag_empty() {
        let entries: Vec<XUriidentityTaggedEntry> = vec![];
        assert!(x_uriidentity_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_uriidentity_tagged_entry_no_tag_match() {
        let e = XUriidentityTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }


    // -- uriidentity extended domain tests ----------------------------------------

    #[test]
    fn y_uriidentity_enum_index() {
        assert_eq!(YUriidentityUriComparison::Equal.index(), 0);
        assert_eq!(YUriidentityUriComparison::CaseInsensitiveEqual.index(), 1);
        assert_eq!(YUriidentityUriComparison::Different.index(), 2);
        assert_eq!(YUriidentityUriComparison::SameAuthority.index(), 3);
    }

    #[test]
    fn y_uriidentity_enum_label() {
        assert_eq!(YUriidentityUriComparison::Equal.label(), "Equal");
        assert_eq!(YUriidentityUriComparison::CaseInsensitiveEqual.label(), "CaseInsensitiveEqual");
        assert_eq!(YUriidentityUriComparison::Different.label(), "Different");
        assert_eq!(YUriidentityUriComparison::SameAuthority.label(), "SameAuthority");
    }

    #[test]
    fn y_uriidentity_enum_all() {
        let all = YUriidentityUriComparison::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_uriidentity_enum_is_default() {
        assert!(YUriidentityUriComparison::Equal.is_default());
        assert!(!YUriidentityUriComparison::SameAuthority.is_default());
    }

    #[test]
    fn y_uriidentity_enum_display() {
        assert_eq!(format!("{}", YUriidentityUriComparison::Equal), "Equal");
    }

    #[test]
    fn y_uriidentity_struct_new() {
        let s = YUriidentityUriNormalizer::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_uriidentity_struct_clear() {
        let mut s = YUriidentityUriNormalizer::new();
        s.scheme_map.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_uriidentity_fingerprint_deterministic() {
        let h1 = y_uriidentity_fingerprint("hello");
        let h2 = y_uriidentity_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_uriidentity_fingerprint("a"), y_uriidentity_fingerprint("b"));
    }

    #[test]
    fn y_uriidentity_truncate_short() {
        assert_eq!(y_uriidentity_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_uriidentity_truncate_long() {
        let r = y_uriidentity_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_uriidentity_normalize_key_basic() {
        assert_eq!(y_uriidentity_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_uriidentity_split_path_basic() {
        let parts = y_uriidentity_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_uriidentity_count_occurrences_basic() {
        assert_eq!(y_uriidentity_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_uriidentity_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_uriidentity_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_uriidentity_in_range_basic() {
        assert!(y_uriidentity_in_range(5, 1, 10));
        assert!(y_uriidentity_in_range(1, 1, 10));
        assert!(y_uriidentity_in_range(10, 1, 10));
        assert!(!y_uriidentity_in_range(0, 1, 10));
        assert!(!y_uriidentity_in_range(11, 1, 10));
    }

    #[test]
    fn y_uriidentity_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_uriidentity_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_uriidentity_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_uriidentity_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- uriidentity Z-extended tests -----------------------------------------------

    #[test]
    fn z_uriidentity_priority_weight() {
        assert_eq!(ZUriidentityPriority::Idle.weight(), 0);
        assert_eq!(ZUriidentityPriority::Normal.weight(), 2);
        assert_eq!(ZUriidentityPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_uriidentity_priority_label() {
        assert_eq!(ZUriidentityPriority::Low.label(), "low");
        assert_eq!(ZUriidentityPriority::High.label(), "high");
    }

    #[test]
    fn z_uriidentity_priority_is_elevated() {
        assert!(!ZUriidentityPriority::Normal.is_elevated());
        assert!(ZUriidentityPriority::High.is_elevated());
        assert!(ZUriidentityPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_uriidentity_priority_display() {
        assert_eq!(format!("{}", ZUriidentityPriority::Idle), "idle");
    }

    #[test]
    fn z_uriidentity_priority_all_asc() {
        let all = ZUriidentityPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZUriidentityPriority::Idle);
        assert_eq!(all[4], ZUriidentityPriority::Realtime);
    }

    #[test]
    fn z_uriidentity_struct_new() {
        let s = ZUriidentityUriCanonicalizer::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_uriidentity_struct_toggled_clone() {
        let s = ZUriidentityUriCanonicalizer::new();
        let t = s.toggled_clone();
        assert_ne!(s.strip_fragment, t.strip_fragment);
    }

    #[test]
    fn z_uriidentity_rolling_hash_deterministic() {
        let h1 = z_uriidentity_rolling_hash(b"test");
        let h2 = z_uriidentity_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_uriidentity_rolling_hash(b"a"), z_uriidentity_rolling_hash(b"b"));
    }

    #[test]
    fn z_uriidentity_pad_to_basic() {
        assert_eq!(z_uriidentity_pad_to("hi", 5), "hi   ");
        assert_eq!(z_uriidentity_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_uriidentity_is_identifier_basic() {
        assert!(z_uriidentity_is_identifier("foo_bar"));
        assert!(z_uriidentity_is_identifier("abc123"));
        assert!(!z_uriidentity_is_identifier(""));
        assert!(!z_uriidentity_is_identifier("has space"));
    }

    #[test]
    fn z_uriidentity_levenshtein_basic() {
        assert_eq!(z_uriidentity_levenshtein("", ""), 0);
        assert_eq!(z_uriidentity_levenshtein("abc", "abc"), 0);
        assert_eq!(z_uriidentity_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_uriidentity_unique_words_basic() {
        let w = z_uriidentity_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_uriidentity_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_uriidentity_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_uriidentity_common_prefix_basic() {
        assert_eq!(z_uriidentity_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_uriidentity_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_uriidentity_struct_clear() {
        let mut s = ZUriidentityUriCanonicalizer::new();
        s.rewrites.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_uriidentity_rolling_hash_empty() {
        let h = z_uriidentity_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_97_push_and_len() {
        let mut rb = super::XbRingBuffer97::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_97_overwrite() {
        let mut rb = super::XbRingBuffer97::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_97_get_out_of_bounds() {
        let rb = super::XbRingBuffer97::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_97_drain_all() {
        let mut rb = super::XbRingBuffer97::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_97_peek_front_back() {
        let mut rb = super::XbRingBuffer97::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_97_clear() {
        let mut rb = super::XbRingBuffer97::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_97_capacity() {
        let rb = super::XbRingBuffer97::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_97_basic() {
        let h = super::xb_fnv1a_97(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_97(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_97_different_inputs() {
        let h1 = super::xb_fnv1a_97(b"abc");
        let h2 = super::xb_fnv1a_97(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_97_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_97(&data);
        let dec = super::xb_rle_decode_97(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_97_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_97(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_97(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_97_values() {
        assert!((super::xb_clamp_97(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_97(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_97(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_97_values() {
        assert!((super::xb_lerp_97(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_97(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_97(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_97_wrap_around_twice() {
        let mut rb = super::XbRingBuffer97::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }

}
