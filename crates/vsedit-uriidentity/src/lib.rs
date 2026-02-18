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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 193
// ---------------------------------------------------------------------------

/// Generic object pool `Xc193Pool<T>`.
pub struct Xc193Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc193Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc193PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc193Pool<T> {
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
    pub fn stats(&self) -> Xc193PoolStats {
        Xc193PoolStats {
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

impl<T> Default for Xc193Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc193Scheduler`.
pub struct Xc193Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc193Scheduler {
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

impl Default for Xc193Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_193 hash for the given byte slice.
pub fn xc_193_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_193 convention.
pub fn xc_193_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe110 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe110Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe110PipelineError {
    pub stage: Xe110Stage,
    pub message: String,
}

impl std::fmt::Display for Xe110PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe110Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe110Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError>>>,
    stage_names: Vec<Xe110Stage>,
}

impl Xe110Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe110Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe110Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe110Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe110Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe110Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe110CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe110CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe110Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe110CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe110CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe110Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe110CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_110_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe110CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_110_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe110CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_110_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> {
    Ok(data)
}

pub fn xe_110_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_110_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_110_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_110_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe110PipelineError> {
    Err(Xe110PipelineError {
        stage: Xe110Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_108: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg108Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg108Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg108Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_108: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg108Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg108Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg108Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg108Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 192).
pub struct Xh192SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh192SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 234 as u64,
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

/// A compact bit set supporting boolean operations (variant 192).
pub struct Xh192BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh192BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 192).
pub struct Xi192Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi192Deque<T> {
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
pub struct Xi192Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi192Interval {
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

/// A simple interval tree (variant 192).
pub struct Xi192IntervalTree {
    xi_intervals: Vec<Xi192Interval>,
}

impl Xi192IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi192Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi192Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi192Interval) -> Vec<&Xi192Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi192Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi192Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi192Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi192Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi192Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi192Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 191) ---

/// Disjoint set / union-find for crate 191.
pub struct Xj191UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj191UnionFind {
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

const XJ191_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 191.
pub struct Xj191BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj191BTreeNode<K, V>>>,
    len: usize,
}

struct Xj191BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj191BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj191BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ191_BTREE_ORDER - 1
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
        let mid = XJ191_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj191BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj191BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj191BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj191BTreeNode::xj_new_leaf();
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


// --- xk_192 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk192SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk192SegmentTree {
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
pub struct Xk192DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk192DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_191).
#[derive(Debug, Clone)]
pub struct Xl191Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl191Rope {
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

/// Suffix array for efficient string searching (xl_191).
#[derive(Debug, Clone)]
pub struct Xl191SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl191SuffixArray {
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


    // ---- xc_ pool / scheduler tests – block 193 ----

    #[test]
    fn xc_193_pool_new_empty() {
        let pool: super::Xc193Pool<i32> = super::Xc193Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_193_pool_release_acquire() {
        let mut pool = super::Xc193Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_193_pool_acquire_empty() {
        let mut pool: super::Xc193Pool<i32> = super::Xc193Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_193_pool_full() {
        let mut pool = super::Xc193Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_193_pool_drain() {
        let mut pool = super::Xc193Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_193_pool_stats() {
        let mut pool = super::Xc193Pool::new(8);
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
    fn xc_193_pool_clear() {
        let mut pool = super::Xc193Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_193_pool_shrink() {
        let mut pool = super::Xc193Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_193_pool_default() {
        let pool: super::Xc193Pool<String> = super::Xc193Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_193_pool_extend() {
        let mut pool = super::Xc193Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_193_pool_retain() {
        let mut pool = super::Xc193Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_193_scheduler_round_robin() {
        let mut sched = super::Xc193Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_193_scheduler_empty() {
        let mut sched = super::Xc193Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_193_scheduler_reset() {
        let mut sched = super::Xc193Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_193_scheduler_add_remove() {
        let mut sched = super::Xc193Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_193_scheduler_targets() {
        let sched = super::Xc193Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_193_hash_empty() {
        assert_eq!(super::xc_193_hash(b""), 5381);
    }

    #[test]
    fn xc_193_hash_data() {
        let h = super::xc_193_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_193_hash(b"hello"), h);
    }

    #[test]
    fn xc_193_reverse_str() {
        assert_eq!(super::xc_193_reverse("abc"), "cba");
        assert_eq!(super::xc_193_reverse(""), "");
    }


    #[test]
    fn xe_110_pipeline_empty() {
        let p = super::Xe110Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_110_pipeline_parse_stage() {
        let p = super::Xe110Pipeline::new()
            .add_parse(super::xe_110_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_110_pipeline_transform_double() {
        let p = super::Xe110Pipeline::new()
            .add_transform(super::xe_110_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_110_pipeline_validate_reverse() {
        let p = super::Xe110Pipeline::new()
            .add_validate(super::xe_110_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_110_pipeline_emit_filter() {
        let p = super::Xe110Pipeline::new()
            .add_emit(super::xe_110_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_110_pipeline_multi_stage() {
        let p = super::Xe110Pipeline::new()
            .add_parse(super::xe_110_pipeline_identity)
            .add_transform(super::xe_110_pipeline_double)
            .add_validate(super::xe_110_pipeline_reverse)
            .add_emit(super::xe_110_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_110_pipeline_error_propagation() {
        let p = super::Xe110Pipeline::new()
            .add_parse(super::xe_110_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe110Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_110_pipeline_compose() {
        let p1 = super::Xe110Pipeline::new()
            .add_parse(super::xe_110_pipeline_identity);
        let p2 = super::Xe110Pipeline::new()
            .add_transform(super::xe_110_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_110_pipeline_error_display() {
        let e = super::Xe110PipelineError {
            stage: super::Xe110Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_110_cache_put_get() {
        let mut c = super::Xe110Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_110_cache_miss() {
        let mut c: super::Xe110Cache<&str, i32> = super::Xe110Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_110_cache_ttl_expiry() {
        let mut c = super::Xe110Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_110_cache_evict() {
        let mut c = super::Xe110Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_110_cache_capacity() {
        let mut c = super::Xe110Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_110_cache_stats() {
        let mut c = super::Xe110Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_110_cache_clear() {
        let mut c = super::Xe110Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_108 graph tests ------------------------------------------------

    #[test]
    fn xg_108_graph_empty() {
        let g = super::Xg108Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_108_graph_add_node() {
        let mut g = super::Xg108Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_108_graph_add_edge() {
        let mut g = super::Xg108Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_108_graph_neighbors() {
        let mut g = super::Xg108Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_108_graph_has_path() {
        let mut g = super::Xg108Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_108_graph_self_path() {
        let g = super::Xg108Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_108_graph_topo_sort() {
        let mut g = super::Xg108Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_108_graph_cycle_detect_false() {
        let mut g = super::Xg108Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_108_graph_cycle_detect_true() {
        let mut g = super::Xg108Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_108 heap tests -------------------------------------------------

    #[test]
    fn xg_108_heap_empty() {
        let h: super::Xg108Heap<i32> = super::Xg108Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_108_heap_push_pop() {
        let mut h = super::Xg108Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_108_heap_peek() {
        let mut h = super::Xg108Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_108_heap_drain_sorted() {
        let mut h = super::Xg108Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_108_heap_merge() {
        let mut a = super::Xg108Heap::new();
        let mut b = super::Xg108Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_108_heap_default() {
        let h: super::Xg108Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_108_graph_default() {
        let g: super::Xg108Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh192_skip_insert_contains() {
        let mut sl = super::Xh192SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh192_skip_remove() {
        let mut sl = super::Xh192SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh192_skip_len() {
        let mut sl = super::Xh192SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh192_skip_range_query() {
        let mut sl = super::Xh192SkipList::xh_new(4);
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
    fn xh192_skip_floor_ceiling() {
        let mut sl = super::Xh192SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh192_skip_rank() {
        let mut sl = super::Xh192SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh192_skip_empty() {
        let sl = super::Xh192SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh192_skip_duplicates() {
        let mut sl = super::Xh192SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh192_bitset_set_test() {
        let mut bs = super::Xh192BitSet::xh_new(256);
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
    fn xh192_bitset_clear_count() {
        let mut bs = super::Xh192BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh192_bitset_and_or_xor() {
        let mut a = super::Xh192BitSet::xh_new(128);
        let mut b = super::Xh192BitSet::xh_new(128);
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
    fn xh192_bitset_iter_ones() {
        let mut bs = super::Xh192BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh192_bitset_first_last() {
        let mut bs = super::Xh192BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh192_bitset_empty() {
        let bs = super::Xh192BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi192_deque_push_pop_back() {
        let mut dq = super::Xi192Deque::xi_new(4);
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
    fn xi192_deque_push_pop_front() {
        let mut dq = super::Xi192Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi192_deque_mixed_ops() {
        let mut dq = super::Xi192Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi192_deque_get_and_split() {
        let mut dq = super::Xi192Deque::xi_new(8);
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
    fn xi192_deque_rotate_left() {
        let mut dq = super::Xi192Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi192_deque_rotate_right() {
        let mut dq = super::Xi192Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi192_deque_grow() {
        let mut dq = super::Xi192Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi192_deque_empty() {
        let dq = super::Xi192Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi192_interval_tree_insert_query() {
        let mut tree = super::Xi192IntervalTree::xi_new();
        tree.xi_insert(super::Xi192Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi192Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi192Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi192_interval_tree_overlap() {
        let mut tree = super::Xi192IntervalTree::xi_new();
        tree.xi_insert(super::Xi192Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi192Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi192Interval::xi_new(12, 20));
        let q = super::Xi192Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi192_interval_tree_remove() {
        let mut tree = super::Xi192IntervalTree::xi_new();
        tree.xi_insert(super::Xi192Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi192Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi192_interval_tree_gaps() {
        let mut tree = super::Xi192IntervalTree::xi_new();
        tree.xi_insert(super::Xi192Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi192Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi192Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi192Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi192Interval::xi_new(8, 10));
    }

    #[test]
    fn xi192_interval_tree_merge() {
        let mut tree = super::Xi192IntervalTree::xi_new();
        tree.xi_insert(super::Xi192Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi192Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi192Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi192Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi192Interval::xi_new(10, 15));
    }

    #[test]
    fn xi192_interval_tree_all() {
        let mut tree = super::Xi192IntervalTree::xi_new();
        tree.xi_insert(super::Xi192Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi192Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi192_interval_tree_empty() {
        let tree = super::Xi192IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi192_interval_tree_contains_point() {
        let iv = super::Xi192Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 191) ---

    #[test]
    fn xj_191_uf_make_and_find() {
        let mut uf = super::Xj191UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_191_uf_union_connected() {
        let mut uf = super::Xj191UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_191_uf_component_count() {
        let mut uf = super::Xj191UnionFind::xj_new();
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
    fn xj_191_uf_component_size() {
        let mut uf = super::Xj191UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_191_uf_largest_component() {
        let mut uf = super::Xj191UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_191_uf_many_elements() {
        let mut uf = super::Xj191UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_191_uf_separate_components() {
        let mut uf = super::Xj191UnionFind::xj_new();
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
    fn xj_191_uf_path_compression() {
        let mut uf = super::Xj191UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_191_bt_insert_get() {
        let mut bt = super::Xj191BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_191_bt_contains_len() {
        let mut bt = super::Xj191BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_191_bt_replace() {
        let mut bt = super::Xj191BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_191_bt_remove() {
        let mut bt = super::Xj191BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_191_bt_keys_values() {
        let mut bt = super::Xj191BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_191_bt_range() {
        let mut bt = super::Xj191BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_191_bt_min_max() {
        let mut bt = super::Xj191BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_191_bt_many_inserts() {
        let mut bt = super::Xj191BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_192 segment tree tests ---

    #[test]
    fn xk_192_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk192SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_192_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk192SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_192_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk192SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_192_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk192SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_192_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk192SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_192_st_single_element() {
        let data = vec![42];
        let st = super::Xk192SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_192_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk192SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_192_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk192SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_192 disjoint intervals tests ---

    #[test]
    fn xk_192_di_add_and_count() {
        let mut di = super::Xk192DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_192_di_merge_overlap() {
        let mut di = super::Xk192DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_192_di_contains() {
        let mut di = super::Xk192DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_192_di_remove() {
        let mut di = super::Xk192DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_192_di_covered_length() {
        let mut di = super::Xk192DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_192_di_gaps() {
        let mut di = super::Xk192DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_192_di_merge_adjacent() {
        let mut di = super::Xk192DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_192_di_empty() {
        let di = super::Xk192DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_191_rope_new_empty() {
        let rope = super::Xl191Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_191_rope_from_str() {
        let rope = super::Xl191Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_191_rope_insert_at() {
        let mut rope = super::Xl191Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_191_rope_delete_range() {
        let mut rope = super::Xl191Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_191_rope_char_at() {
        let rope = super::Xl191Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_191_rope_split_concat() {
        let rope = super::Xl191Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_191_rope_line_count() {
        let rope = super::Xl191Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_191_rope_line_at() {
        let rope = super::Xl191Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_191_sa_build_and_search() {
        let sa = super::Xl191SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_191_sa_count() {
        let sa = super::Xl191SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_191_sa_longest_repeated() {
        let sa = super::Xl191SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_191_sa_all_positions() {
        let sa = super::Xl191SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_191_sa_len() {
        let sa = super::Xl191SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_191_sa_empty() {
        let sa = super::Xl191SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_191_rope_slice() {
        let rope = super::Xl191Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_191_sa_search_start() {
        let sa = super::Xl191SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
