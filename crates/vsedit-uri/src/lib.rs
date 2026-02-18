//! URI parsing and resource identification.
//!
//! Equivalent to VS Code's `vs/base/common/uri.ts`. The [`VsUri`] type is the
//! fundamental resource identifier used throughout the editor.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Common URI schemes
// ---------------------------------------------------------------------------

/// File-system resource (`file://`).
pub const FILE: &str = "file";
/// Untitled (unsaved) editor resource.
pub const UNTITLED: &str = "untitled";
/// Internal VS Code resource.
pub const VSCODE: &str = "vscode";
/// Remote VS Code resource.
pub const VSCODE_REMOTE: &str = "vscode-remote";
/// HTTP resource.
pub const HTTP: &str = "http";
/// HTTPS resource.
pub const HTTPS: &str = "https";
/// Mailto link.
pub const MAILTO: &str = "mailto";
/// Data URI.
pub const DATA: &str = "data";
/// Editor command.
pub const COMMAND: &str = "command";
/// Settings resource.
pub const VSCODE_SETTINGS: &str = "vscode-settings";
/// User-data resource.
pub const VSCODE_USERDATA: &str = "vscode-userdata";

// ---------------------------------------------------------------------------
// Percent-encoding helpers
// ---------------------------------------------------------------------------

/// Characters that are never percent-encoded in the *path* component.
fn is_path_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*'
                | b'+' | b',' | b';' | b'=' | b':' | b'@' | b'/'
        )
}

/// Percent-encode `input` for use in the path component of a URI.
fn encode_path(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if is_path_unreserved(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from(HEX_UPPER[b as usize >> 4]));
            out.push(char::from(HEX_UPPER[b as usize & 0xF]));
        }
    }
    out
}

/// Percent-encode for the authority component.
fn encode_authority(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*'
                    | b'+' | b',' | b';' | b'=' | b':' | b'@' | b'[' | b']'
            )
        {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from(HEX_UPPER[b as usize >> 4]));
            out.push(char::from(HEX_UPPER[b as usize & 0xF]));
        }
    }
    out
}

/// Percent-encode for the query component (preserves `=`, `&`, `+` etc.).
fn encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*'
                    | b'+' | b',' | b';' | b'=' | b':' | b'@' | b'/' | b'?'
            )
        {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from(HEX_UPPER[b as usize >> 4]));
            out.push(char::from(HEX_UPPER[b as usize & 0xF]));
        }
    }
    out
}

/// Percent-encode for the fragment component.
fn encode_fragment(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*'
                    | b'+' | b',' | b';' | b'=' | b':' | b'@' | b'/' | b'?'
            )
        {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from(HEX_UPPER[b as usize >> 4]));
            out.push(char::from(HEX_UPPER[b as usize & 0xF]));
        }
    }
    out
}

const HEX_UPPER: [u8; 16] = *b"0123456789ABCDEF";

/// Decode percent-encoded bytes in a string.
fn percent_decode(input: &str) -> String {
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
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// UriChanges — used by `VsUri::with`
// ---------------------------------------------------------------------------

/// Describes which parts of a URI to replace when calling [`VsUri::with`].
#[derive(Debug, Default, Clone)]
pub struct UriChanges {
    pub scheme: Option<String>,
    pub authority: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

// ---------------------------------------------------------------------------
// VsUri
// ---------------------------------------------------------------------------

/// A Universal Resource Identifier representing a resource in the editor.
///
/// Modelled after VS Code's `URI` class; the five components follow RFC 3986:
///
/// ```text
/// scheme ":" ["//" authority] path ["?" query] ["#" fragment]
/// ```
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct VsUri {
    /// URI scheme (e.g. `file`, `https`).
    pub scheme: String,
    /// Authority component (e.g. `example.com`).
    pub authority: String,
    /// Path component (always uses forward slashes).
    pub path: String,
    /// Query component (without the leading `?`).
    pub query: String,
    /// Fragment component (without the leading `#`).
    pub fragment: String,
}

// --- Core API ---------------------------------------------------------------

impl VsUri {
    // -- Constructors --------------------------------------------------------

    /// Create a URI from all five components.
    ///
    /// ```
    /// # use vsedit_uri::VsUri;
    /// let uri = VsUri::from_components("https", "example.com", "/path", "q=1", "frag");
    /// assert_eq!(uri.scheme, "https");
    /// ```
    pub fn from_components(
        scheme: &str,
        authority: &str,
        path: &str,
        query: &str,
        fragment: &str,
    ) -> Self {
        Self {
            scheme: scheme.to_string(),
            authority: authority.to_string(),
            path: vsedit_path::to_forward_slashes(path),
            query: query.to_string(),
            fragment: fragment.to_string(),
        }
    }

    /// Create a `file://` URI from a filesystem path.
    ///
    /// On all platforms backslashes are normalised to forward slashes.
    /// Windows drive-letter paths (`C:\…`) are lowercased in the URI.
    ///
    /// ```
    /// # use vsedit_uri::VsUri;
    /// let uri = VsUri::file("/home/user/file.rs");
    /// assert_eq!(uri.scheme, "file");
    /// assert_eq!(uri.path, "/home/user/file.rs");
    /// ```
    pub fn file(path: &str) -> Self {
        let path = vsedit_path::to_forward_slashes(path);

        // Ensure the path starts with `/`.
        let path = if path.starts_with('/') {
            path
        } else {
            format!("/{path}")
        };

        // Lowercase Windows drive letter for consistency.
        let path = lowercase_drive_letter(&path);

        Self {
            scheme: FILE.to_string(),
            authority: String::new(),
            path,
            query: String::new(),
            fragment: String::new(),
        }
    }

    /// Parse a URI string into a [`VsUri`].
    ///
    /// Handles `file://`, authority-based, and opaque URIs.
    ///
    /// ```
    /// # use vsedit_uri::VsUri;
    /// let uri = VsUri::parse("https://example.com/path?q=1#frag");
    /// assert_eq!(uri.scheme, "https");
    /// assert_eq!(uri.authority, "example.com");
    /// assert_eq!(uri.path, "/path");
    /// assert_eq!(uri.query, "q=1");
    /// assert_eq!(uri.fragment, "frag");
    /// ```
    pub fn parse(value: &str) -> Self {
        // Fast-path: empty string.
        if value.is_empty() {
            return Self::from_components("", "", "", "", "");
        }

        // Use the `url` crate for well-formed absolute URIs.
        if let Ok(parsed) = url::Url::parse(value) {
            let scheme = parsed.scheme().to_string();
            let authority = parsed
                .host_str()
                .map(|h| {
                    if let Some(port) = parsed.port() {
                        format!("{h}:{port}")
                    } else {
                        h.to_string()
                    }
                })
                .unwrap_or_default();
            let path = percent_decode(parsed.path());
            let query = parsed.query().unwrap_or("").to_string();
            let fragment = parsed.fragment().unwrap_or("").to_string();

            return Self {
                scheme,
                authority,
                path,
                query,
                fragment,
            };
        }

        // Fallback: manual parse for things like `untitled:Untitled-1`.
        parse_uri_manual(value)
    }

    /// Create a new URI based on `base` with selected parts replaced.
    ///
    /// ```
    /// # use vsedit_uri::{VsUri, UriChanges};
    /// let base = VsUri::parse("https://example.com/old");
    /// let changed = VsUri::with(&base, UriChanges {
    ///     path: Some("/new".into()),
    ///     ..Default::default()
    /// });
    /// assert_eq!(changed.path, "/new");
    /// assert_eq!(changed.authority, "example.com");
    /// ```
    pub fn with(base: &Self, changes: UriChanges) -> Self {
        Self {
            scheme: changes.scheme.unwrap_or_else(|| base.scheme.clone()),
            authority: changes.authority.unwrap_or_else(|| base.authority.clone()),
            path: changes
                .path
                .map(|p| vsedit_path::to_forward_slashes(&p))
                .unwrap_or_else(|| base.path.clone()),
            query: changes.query.unwrap_or_else(|| base.query.clone()),
            fragment: changes.fragment.unwrap_or_else(|| base.fragment.clone()),
        }
    }

    // -- Queries -------------------------------------------------------------

    /// Returns `true` if this is a `file://` URI.
    pub fn is_file(&self) -> bool {
        vsedit_strings::equals_ignore_case(&self.scheme, FILE)
    }

    /// Returns `true` if this is an `untitled:` URI.
    pub fn is_untitled(&self) -> bool {
        vsedit_strings::equals_ignore_case(&self.scheme, UNTITLED)
    }

    /// Convert a `file://` URI back to a native filesystem path.
    ///
    /// On Unix the result is a simple decoded path.  On Windows the leading
    /// slash is stripped and the drive letter is upper-cased.
    ///
    /// ```
    /// # use vsedit_uri::VsUri;
    /// let uri = VsUri::file("/home/user/file.rs");
    /// assert_eq!(uri.fs_path(), "/home/user/file.rs");
    /// ```
    pub fn fs_path(&self) -> String {
        uri_path_to_fs_path(&self.path, cfg!(windows))
    }

    /// Serialize the URI to a well-formed URI string (percent-encoded where
    /// required).
    pub fn to_uri_string(&self) -> String {
        format_uri(self)
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for VsUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uri_string())
    }
}

// ---------------------------------------------------------------------------
// Serde
// ---------------------------------------------------------------------------

impl Serialize for VsUri {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Serialize as the URI string representation (matches VS Code JSON).
        serializer.serialize_str(&self.to_uri_string())
    }
}

impl<'de> Deserialize<'de> for VsUri {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(VsUri::parse(&s))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Lowercase the drive letter in a path like `/C:/…` → `/c:/…`.
fn lowercase_drive_letter(path: &str) -> String {
    let bytes = path.as_bytes();
    if bytes.len() >= 3
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && bytes[2] == b':'
    {
        let mut s = String::with_capacity(path.len());
        s.push('/');
        s.push((bytes[1] as char).to_ascii_lowercase());
        s.push_str(&path[2..]);
        s
    } else {
        path.to_string()
    }
}

/// Convert a URI path component back to a filesystem path.
fn uri_path_to_fs_path(path: &str, is_windows: bool) -> String {
    let decoded = percent_decode(path);

    if is_windows {
        // Strip leading `/` from `/c:/…`
        let stripped = decoded.strip_prefix('/').unwrap_or(&decoded);
        // Upper-case the drive letter.
        let bytes = stripped.as_bytes();
        let result = if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            let mut s = String::with_capacity(stripped.len());
            s.push((bytes[0] as char).to_ascii_uppercase());
            s.push_str(&stripped[1..]);
            s
        } else {
            stripped.to_string()
        };
        vsedit_path::to_back_slashes(&result)
    } else {
        decoded
    }
}

/// Format a [`VsUri`] into a proper URI string.
fn format_uri(uri: &VsUri) -> String {
    let mut out = String::with_capacity(64);

    // scheme
    if !uri.scheme.is_empty() {
        out.push_str(&uri.scheme);
        out.push(':');
    }

    // authority
    if !uri.authority.is_empty() {
        out.push_str("//");
        out.push_str(&encode_authority(&uri.authority));
    } else if uri.scheme == FILE {
        out.push_str("//");
    }

    // path
    if !uri.path.is_empty() {
        out.push_str(&encode_path(&uri.path));
    }

    // query
    if !uri.query.is_empty() {
        out.push('?');
        out.push_str(&encode_query(&uri.query));
    }

    // fragment
    if !uri.fragment.is_empty() {
        out.push('#');
        out.push_str(&encode_fragment(&uri.fragment));
    }

    out
}

/// Manual URI parser for edge-case strings the `url` crate rejects.
fn parse_uri_manual(value: &str) -> VsUri {
    let (scheme, rest) = match value.find(':') {
        Some(idx) => (&value[..idx], &value[idx + 1..]),
        None => {
            return VsUri::from_components("", "", value, "", "");
        }
    };

    // Split off fragment.
    let (rest, fragment) = match rest.rfind('#') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };

    // Split off query.
    let (rest, query) = match rest.find('?') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };

    // Authority.
    let (authority, path) = if let Some(after_slashes) = rest.strip_prefix("//") {
        match after_slashes.find('/') {
            Some(idx) => (&after_slashes[..idx], &after_slashes[idx..]),
            None => (after_slashes, ""),
        }
    } else {
        ("", rest)
    };

    VsUri {
        scheme: scheme.to_string(),
        authority: percent_decode(authority),
        path: percent_decode(path),
        query: query.to_string(),
        fragment: fragment.to_string(),
    }
}


// ---------------------------------------------------------------------------
// URI resolution — resolve relative URIs against a base
// ---------------------------------------------------------------------------

impl VsUri {
    /// Resolve a relative path reference against this URI as the base.
    ///
    /// ```
    /// # use vsedit_uri::VsUri;
    /// let base = VsUri::parse("https://example.com/a/b/c");
    /// let resolved = base.resolve("../d");
    /// assert_eq!(resolved.path, "/a/d");
    /// ```
    pub fn resolve(&self, relative: &str) -> Self {
        if relative.is_empty() {
            return self.clone();
        }

        // If relative is a full URI, just parse it.
        if relative.contains("://") || relative.starts_with("data:") || relative.starts_with("mailto:") {
            return VsUri::parse(relative);
        }

        // Split off fragment from relative
        let (rel_path_query, fragment) = match relative.find('#') {
            Some(idx) => (&relative[..idx], &relative[idx + 1..]),
            None => (relative, ""),
        };

        // Split off query from relative
        let (rel_path, query) = match rel_path_query.find('?') {
            Some(idx) => (&rel_path_query[..idx], &rel_path_query[idx + 1..]),
            None => (rel_path_query, ""),
        };

        let new_path = if rel_path.starts_with('/') {
            // Absolute path — use directly
            normalize_path(rel_path)
        } else {
            // Relative path — merge with base
            let base_dir = match self.path.rfind('/') {
                Some(idx) => &self.path[..idx + 1],
                None => "/",
            };
            let merged = format!("{}{}", base_dir, rel_path);
            normalize_path(&merged)
        };

        VsUri {
            scheme: self.scheme.clone(),
            authority: self.authority.clone(),
            path: new_path,
            query: if query.is_empty() && rel_path.is_empty() {
                self.query.clone()
            } else {
                query.to_string()
            },
            fragment: fragment.to_string(),
        }
    }
}

/// Normalize a path by resolving `.` and `..` segments.
fn normalize_path(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "." => {}
            ".." => {
                // Don't pop past root
                if segments.len() > 1 {
                    segments.pop();
                }
            }
            _ => segments.push(seg),
        }
    }
    let result = segments.join("/");
    if result.is_empty() {
        "/".to_string()
    } else {
        result
    }
}

// ---------------------------------------------------------------------------
// URI query parameter parsing & building
// ---------------------------------------------------------------------------

/// Parsed query parameters from a URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParams {
    params: Vec<(String, String)>,
}

impl QueryParams {
    /// Parse query parameters from a query string (without the leading `?`).
    pub fn parse(query: &str) -> Self {
        let mut params = Vec::new();
        if query.is_empty() {
            return Self { params };
        }
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = match pair.find('=') {
                Some(idx) => (
                    percent_decode(&pair[..idx]),
                    percent_decode(&pair[idx + 1..]),
                ),
                None => (percent_decode(pair), String::new()),
            };
            params.push((key, value));
        }
        Self { params }
    }

    /// Parse query parameters directly from a [`VsUri`].
    pub fn from_uri(uri: &VsUri) -> Self {
        Self::parse(&uri.query)
    }

    /// Get the first value for a given key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Get all values for a given key.
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        self.params
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Returns true if the given key is present.
    pub fn has(&self, key: &str) -> bool {
        self.params.iter().any(|(k, _)| k == key)
    }

    /// Return the number of parameters.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Return true if there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Build the query string (without the leading `?`).
    pub fn to_query_string(&self) -> String {
        self.params
            .iter()
            .map(|(k, v)| {
                if v.is_empty() {
                    encode_query(k)
                } else {
                    format!("{}={}", encode_query(k), encode_query(v))
                }
            })
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Add a key-value pair.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.params.push((key.into(), value.into()));
    }

    /// Remove all entries with the given key. Returns the number removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.params.len();
        self.params.retain(|(k, _)| k != key);
        before - self.params.len()
    }

    /// Return all keys (may contain duplicates).
    pub fn keys(&self) -> Vec<&str> {
        self.params.iter().map(|(k, _)| k.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// UriBuilder — fluent builder for constructing URIs piece by piece
// ---------------------------------------------------------------------------

/// Fluent builder for constructing [`VsUri`] instances.
///
/// ```
/// # use vsedit_uri::UriBuilder;
/// let uri = UriBuilder::new()
///     .scheme("https")
///     .authority("example.com")
///     .path("/api/v1")
///     .query("page=1")
///     .fragment("top")
///     .build();
/// assert_eq!(uri.scheme, "https");
/// ```
#[derive(Debug, Default, Clone)]
pub struct UriBuilder {
    scheme: String,
    authority: String,
    path: String,
    query: String,
    fragment: String,
}

impl UriBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start from an existing URI.
    pub fn from_uri(uri: &VsUri) -> Self {
        Self {
            scheme: uri.scheme.clone(),
            authority: uri.authority.clone(),
            path: uri.path.clone(),
            query: uri.query.clone(),
            fragment: uri.fragment.clone(),
        }
    }

    /// Set the scheme (e.g. `"https"`, `"file"`).
    pub fn scheme(mut self, scheme: &str) -> Self {
        self.scheme = scheme.to_string();
        self
    }

    /// Set the authority (e.g. `"example.com:8080"`).
    pub fn authority(mut self, authority: &str) -> Self {
        self.authority = authority.to_string();
        self
    }

    /// Set the path (e.g. `"/api/v1/users"`).
    pub fn path(mut self, path: &str) -> Self {
        self.path = vsedit_path::to_forward_slashes(path);
        self
    }

    /// Set the raw query string (without leading `?`).
    pub fn query(mut self, query: &str) -> Self {
        self.query = query.to_string();
        self
    }

    /// Set the query from a [`QueryParams`] instance.
    pub fn query_params(mut self, params: &QueryParams) -> Self {
        self.query = params.to_query_string();
        self
    }

    /// Set the fragment (without leading `#`).
    pub fn fragment(mut self, fragment: &str) -> Self {
        self.fragment = fragment.to_string();
        self
    }

    /// Build the final [`VsUri`].
    pub fn build(self) -> VsUri {
        VsUri {
            scheme: self.scheme,
            authority: self.authority,
            path: self.path,
            query: self.query,
            fragment: self.fragment,
        }
    }
}

// ---------------------------------------------------------------------------
// UriMatcher — glob-style pattern matching for URIs
// ---------------------------------------------------------------------------

/// Matches URIs against patterns with glob-style wildcards.
///
/// Supported wildcards:
/// - `*` matches any characters within a single path segment
/// - `**` matches any characters across multiple path segments
/// - Wildcards can appear in scheme, authority, or path
///
/// ```
/// # use vsedit_uri::{UriMatcher, VsUri};
/// let m = UriMatcher::new("https://example.com/api/**");
/// assert!(m.matches(&VsUri::parse("https://example.com/api/v1/users")));
/// ```
#[derive(Debug, Clone)]
pub struct UriMatcher {
    scheme_pattern: String,
    authority_pattern: String,
    path_pattern: String,
}

impl UriMatcher {
    /// Create a new matcher from a pattern string.
    ///
    /// The pattern is parsed as `scheme://authority/path` where any component
    /// may contain `*` or `**` wildcards.
    pub fn new(pattern: &str) -> Self {
        let parsed = VsUri::parse(pattern);
        Self {
            scheme_pattern: parsed.scheme,
            authority_pattern: parsed.authority,
            path_pattern: parsed.path,
        }
    }

    /// Test whether a URI matches this pattern.
    pub fn matches(&self, uri: &VsUri) -> bool {
        glob_match(&self.scheme_pattern, &uri.scheme)
            && glob_match(&self.authority_pattern, &uri.authority)
            && glob_match_path(&self.path_pattern, &uri.path)
    }
}

/// Simple glob match for non-path components (`*` matches any chars).
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    pattern == value
}

/// Glob match for path components with `*` and `**` support.
fn glob_match_path(pattern: &str, path: &str) -> bool {
    let pat_segs: Vec<&str> = pattern.split('/').collect();
    let path_segs: Vec<&str> = path.split('/').collect();
    glob_match_segments(&pat_segs, &path_segs)
}

fn glob_match_segments(pat: &[&str], path: &[&str]) -> bool {
    let mut pi = 0;
    let mut si = 0;
    let mut star_pi = None;
    let mut star_si = None;

    while si < path.len() {
        if pi < pat.len() && pat[pi] == "**" {
            star_pi = Some(pi);
            star_si = Some(si);
            pi += 1;
        } else if pi < pat.len() && segment_matches(pat[pi], path[si]) {
            pi += 1;
            si += 1;
        } else if let (Some(sp), Some(ss)) = (star_pi, star_si) {
            pi = sp + 1;
            let new_ss = ss + 1;
            star_si = Some(new_ss);
            si = new_ss;
        } else {
            return false;
        }
    }

    while pi < pat.len() && pat[pi] == "**" {
        pi += 1;
    }

    pi == pat.len()
}

/// Match a single path segment against a pattern segment (`*` = any segment).
fn segment_matches(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    pattern == segment
}

// ---------------------------------------------------------------------------
// resolve_relative — free function wrapper
// ---------------------------------------------------------------------------

/// Resolve a relative URI reference against a base URI.
///
/// This is a convenience wrapper around [`VsUri::resolve`].
///
/// ```
/// # use vsedit_uri::{VsUri, resolve_relative};
/// let base = VsUri::parse("https://example.com/a/b/c");
/// let resolved = resolve_relative(&base, "../d");
/// assert_eq!(resolved.path, "/a/d");
/// ```
pub fn resolve_relative(base: &VsUri, relative: &str) -> VsUri {
    base.resolve(relative)
}

// ---------------------------------------------------------------------------
// From / Into conversions
// ---------------------------------------------------------------------------

impl From<&str> for VsUri {
    fn from(s: &str) -> Self {
        VsUri::parse(s)
    }
}

impl From<String> for VsUri {
    fn from(s: String) -> Self {
        VsUri::parse(&s)
    }
}

impl From<VsUri> for String {
    fn from(uri: VsUri) -> Self {
        uri.to_uri_string()
    }
}

// ---------------------------------------------------------------------------
// Display for QueryParams
// ---------------------------------------------------------------------------

impl fmt::Display for QueryParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_query_string())
    }
}

// ---------------------------------------------------------------------------
// Iterator support for QueryParams
// ---------------------------------------------------------------------------

impl QueryParams {
    /// Iterate over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.params.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

// ---------------------------------------------------------------------------
// Data URI encoding/decoding
// ---------------------------------------------------------------------------

/// A decoded data URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataUri {
    pub mime_type: String,
    pub data: Vec<u8>,
    pub is_base64: bool,
}

impl DataUri {
    /// Decode a data URI string into its components.
    ///
    /// Supports both plain text (`data:text/plain,Hello`) and base64
    /// (`data:text/plain;base64,SGVsbG8=`) forms.
    pub fn decode(uri_str: &str) -> Option<Self> {
        let rest = uri_str.strip_prefix("data:")?;

        let (header, data_part) = match rest.find(',') {
            Some(idx) => (&rest[..idx], &rest[idx + 1..]),
            None => return None,
        };

        let is_base64 = header.ends_with(";base64");
        let mime_type = if is_base64 {
            header.strip_suffix(";base64").unwrap_or(header)
        } else {
            header
        };

        let data = if is_base64 {
            base64_decode(data_part)?
        } else {
            percent_decode(data_part).into_bytes()
        };

        Some(DataUri {
            mime_type: mime_type.to_string(),
            data,
            is_base64,
        })
    }

    /// Encode this data URI back to a string.
    pub fn encode(&self) -> String {
        if self.is_base64 {
            format!("data:{};base64,{}", self.mime_type, base64_encode(&self.data))
        } else {
            format!(
                "data:{},{}",
                self.mime_type,
                encode_path(&String::from_utf8_lossy(&self.data))
            )
        }
    }
}

/// Simple base64 decoder (RFC 4648).
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let table = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' | b'\n' | b'\r' | b' ' => None,
            _ => None,
        }
    };

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=' && b != b'\n' && b != b'\r' && b != b' ')
        .collect();

    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &byte in &filtered {
        let val = table(byte)?;
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Simple base64 encoder (RFC 4648).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

// ---------------------------------------------------------------------------
// URI analysis and manipulation helpers
// ---------------------------------------------------------------------------

/// Extract the file extension from a URI's path (without the leading dot).
/// Returns `None` if no extension is present.
pub fn uri_extension(uri: &VsUri) -> Option<&str> {
    let path = &uri.path;
    if let Some(dot) = path.rfind('.') {
        let ext = &path[dot + 1..];
        if ext.is_empty() || ext.contains('/') {
            None
        } else {
            Some(ext)
        }
    } else {
        None
    }
}

/// Extract the file name (last path segment) from a URI.
pub fn uri_filename(uri: &VsUri) -> &str {
    uri.path.rsplit('/').next().unwrap_or(&uri.path)
}

/// Return the parent path of a URI (everything up to the last `/`).
pub fn uri_parent(uri: &VsUri) -> &str {
    if let Some(pos) = uri.path.rfind('/') {
        &uri.path[..pos]
    } else {
        ""
    }
}

/// Split a URI path into individual segments.
pub fn path_segments(uri: &VsUri) -> Vec<&str> {
    uri.path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Check if two URIs share the same origin (scheme + authority).
pub fn same_origin(a: &VsUri, b: &VsUri) -> bool {
    a.scheme == b.scheme && a.authority == b.authority
}

/// Compute a relative path from `base` to `target` if they share the same
/// origin. Returns `None` if origins differ.
pub fn relative_path(base: &VsUri, target: &VsUri) -> Option<String> {
    if !same_origin(base, target) {
        return None;
    }
    let base_parts: Vec<&str> = base.path.split('/').filter(|s| !s.is_empty()).collect();
    let target_parts: Vec<&str> = target.path.split('/').filter(|s| !s.is_empty()).collect();
    let common = base_parts
        .iter()
        .zip(target_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let ups = base_parts.len().saturating_sub(common + 1); // -1 for the base file
    let mut result = String::new();
    for _ in 0..ups {
        result.push_str("../");
    }
    result.push_str(&target_parts[common..].join("/"));
    Some(result)
}

/// Decode percent-encoded sequences in a string (public re-export).
pub fn pct_decode(input: &str) -> String {
    percent_decode(input)
}

// ---------------------------------------------------------------------------
// UriTemplate – RFC 6570-inspired URI templates
// ---------------------------------------------------------------------------

/// Simple URI template expansion (subset of RFC 6570 Level 1).
#[derive(Debug, Clone)]
pub struct UriTemplate {
    template: String,
}

impl UriTemplate {
    pub fn new(template: impl Into<String>) -> Self {
        Self { template: template.into() }
    }

    /// Expand the template, replacing `{name}` placeholders with values.
    pub fn expand(&self, vars: &[(&str, &str)]) -> String {
        let mut result = self.template.clone();
        for (name, value) in vars {
            let placeholder = format!("{{{name}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// List variable names found in the template.
    pub fn variable_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = self.template.as_str();
        while let Some(start) = rest.find('{') {
            if let Some(end) = rest[start..].find('}') {
                let name = &rest[start + 1..start + end];
                if !name.is_empty() {
                    names.push(name.to_string());
                }
                rest = &rest[start + end + 1..];
            } else {
                break;
            }
        }
        names
    }

    /// Whether all variables have been provided.
    pub fn is_fully_expanded(&self, vars: &[(&str, &str)]) -> bool {
        self.variable_names().iter().all(|n| vars.iter().any(|(k, _)| k == n))
    }

    /// The raw template string.
    pub fn as_str(&self) -> &str {
        &self.template
    }
}

impl fmt::Display for UriTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UriTemplate(\"{}\")", self.template)
    }
}

// ---------------------------------------------------------------------------
// UriNormalizer – canonical form normalization
// ---------------------------------------------------------------------------

/// Normalizes URIs to canonical form.
pub struct UriNormalizer;

impl UriNormalizer {
    /// Normalize a URI string to canonical form:
    /// - lowercase scheme and host
    /// - remove default ports (80 for http, 443 for https)
    /// - remove trailing slash from path (unless it's the root)
    /// - remove empty fragment
    pub fn normalize(uri: &VsUri) -> VsUri {
        let scheme = uri.scheme.to_lowercase();
        let authority = uri.authority.to_lowercase();

        // Remove default port from authority
        let authority = Self::strip_default_port(&scheme, &authority);

        // Normalize path: remove trailing slash unless root
        let path = if uri.path.len() > 1 && uri.path.ends_with('/') {
            uri.path.trim_end_matches('/').to_string()
        } else {
            uri.path.clone()
        };

        // Remove empty fragment
        let fragment = if uri.fragment.is_empty() {
            String::new()
        } else {
            uri.fragment.clone()
        };

        VsUri::from_components(&scheme, &authority, &path, &uri.query, &fragment)
    }

    fn strip_default_port(scheme: &str, authority: &str) -> String {
        match scheme {
            "http" => authority.strip_suffix(":80").unwrap_or(authority).to_string(),
            "https" => authority.strip_suffix(":443").unwrap_or(authority).to_string(),
            _ => authority.to_string(),
        }
    }

    /// Check if two URIs are equivalent after normalization.
    pub fn are_equivalent(a: &VsUri, b: &VsUri) -> bool {
        let na = Self::normalize(a);
        let nb = Self::normalize(b);
        na.to_uri_string() == nb.to_uri_string()
    }
}

// ---------------------------------------------------------------------------
// UriQueryBuilder – query string construction
// ---------------------------------------------------------------------------

/// Builder for constructing URI query strings with proper encoding.
#[derive(Debug, Clone)]
pub struct UriQueryBuilder {
    params: Vec<(String, String)>,
}

impl UriQueryBuilder {
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Add a key-value parameter.
    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.push((key.into(), value.into()));
        self
    }

    /// Build the query string (without leading '?').
    pub fn build(&self) -> String {
        self.params
            .iter()
            .map(|(k, v)| format!("{}={}", Self::encode(k), Self::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Simple percent-encoding for query parameters.
    fn encode(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for &b in input.as_bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                out.push(b as char);
            } else if b == b' ' {
                out.push('+');
            } else {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(b & 0xF) as usize]));
            }
        }
        out
    }

    /// Number of parameters.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }
}

impl Default for UriQueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UriQueryBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.build())
    }
}

// ---------------------------------------------------------------------------
// URI authority parser with userinfo
// ---------------------------------------------------------------------------

/// Parsed URI authority component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriAuthority {
    pub userinfo: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

impl UriAuthority {
    /// Parse an authority string like "user:pass@host:port".
    pub fn parse(authority: &str) -> Self {
        let (userinfo, host_port) = if let Some(at_pos) = authority.find('@') {
            (Some(authority[..at_pos].to_string()), &authority[at_pos + 1..])
        } else {
            (None, authority)
        };

        // Check for IPv6 bracket notation
        let (host, port) = if host_port.starts_with('[') {
            if let Some(bracket_end) = host_port.find(']') {
                let host = &host_port[1..bracket_end];
                let rest = &host_port[bracket_end + 1..];
                let port = rest.strip_prefix(':').and_then(|p| p.parse().ok());
                (host.to_string(), port)
            } else {
                (host_port.to_string(), None)
            }
        } else if let Some(colon) = host_port.rfind(':') {
            let host = &host_port[..colon];
            let port = host_port[colon + 1..].parse().ok();
            (host.to_string(), port)
        } else {
            (host_port.to_string(), None)
        };

        Self { userinfo, host, port }
    }

    /// Reconstruct the authority string.
    pub fn to_string_repr(&self) -> String {
        let mut s = String::new();
        if let Some(ref ui) = self.userinfo {
            s.push_str(ui);
            s.push('@');
        }
        s.push_str(&self.host);
        if let Some(p) = self.port {
            s.push(':');
            s.push_str(&p.to_string());
        }
        s
    }

    /// Whether the authority has user information.
    pub fn has_userinfo(&self) -> bool {
        self.userinfo.is_some()
    }
}

impl fmt::Display for UriAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_repr())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// UriEncodeDecoder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UriEncodeDecoder {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl UriEncodeDecoder {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for UriEncodeDecoder {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for UriEncodeDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "UriEncodeDecoder({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// UriCanonicalComparator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UriCanonicalComparator {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl UriCanonicalComparator {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for UriCanonicalComparator {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for UriCanonicalComparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "UriCanonicalComparator({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// UriEncodeDecoderSnapshot — point-in-time snapshot of UriEncodeDecoder state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UriEncodeDecoderSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl UriEncodeDecoderSnapshot {
    pub fn capture(source: &UriEncodeDecoder, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for UriEncodeDecoderSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// UriCanonicalComparatorStats — aggregate statistics for UriCanonicalComparator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct UriCanonicalComparatorStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl UriCanonicalComparatorStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for UriCanonicalComparatorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// UriEncodeDecoderConfig — configuration for UriEncodeDecoder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UriEncodeDecoderConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl UriEncodeDecoderConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for UriEncodeDecoderConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for UriEncodeDecoderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// UriTemplateExpander – stateful URI template expansion
// ---------------------------------------------------------------------------

/// A stateful URI template expander that allows incremental variable
/// registration and repeated expansion.
#[derive(Debug, Clone)]
pub struct UriTemplateExpander {
    template: String,
    variables: HashMap<String, String>,
}

impl UriTemplateExpander {
    /// Create a new expander for the given template string.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
            variables: HashMap::new(),
        }
    }

    /// Register a variable value for substitution.
    pub fn register_variable(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self {
        self.variables.insert(name.into(), value.into());
        self
    }

    /// Register multiple variables at once.
    pub fn register_all(&mut self, vars: &[(&str, &str)]) -> &mut Self {
        for (k, v) in vars {
            self.variables.insert((*k).to_string(), (*v).to_string());
        }
        self
    }

    /// Expand the template, replacing `{var}` placeholders with registered
    /// values. Unresolved placeholders are left as-is.
    pub fn expand(&self) -> String {
        let mut result = self.template.clone();
        for (name, value) in &self.variables {
            let placeholder = format!("{{{name}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Return the names of variables found in the template that have not been
    /// registered yet.
    pub fn unresolved_vars(&self) -> Vec<String> {
        let mut unresolved = Vec::new();
        let mut rest = self.template.as_str();
        while let Some(start) = rest.find('{') {
            if let Some(end) = rest[start..].find('}') {
                let name = &rest[start + 1..start + end];
                if !name.is_empty() && !self.variables.contains_key(name) {
                    unresolved.push(name.to_string());
                }
                rest = &rest[start + end + 1..];
            } else {
                break;
            }
        }
        unresolved
    }

    /// Whether every placeholder in the template has a registered value.
    pub fn is_fully_resolved(&self) -> bool {
        self.unresolved_vars().is_empty()
    }

    /// Return the number of registered variables.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Remove a previously registered variable.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.variables.remove(name).is_some()
    }

    /// Clear all registered variables.
    pub fn clear_variables(&mut self) {
        self.variables.clear();
    }

    /// The raw template string.
    pub fn template(&self) -> &str {
        &self.template
    }
}

impl fmt::Display for UriTemplateExpander {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UriTemplateExpander({} vars, {} unresolved)",
            self.variable_count(),
            self.unresolved_vars().len()
        )
    }
}

// ---------------------------------------------------------------------------
// UriPathManipulator – path-level operations on VsUri
// ---------------------------------------------------------------------------

/// Utility for manipulating the path component of URIs.
pub struct UriPathManipulator;

impl UriPathManipulator {
    /// Join a relative path onto a URI's existing path.
    pub fn join(uri: &VsUri, relative: &str) -> VsUri {
        let base = if uri.path.ends_with('/') {
            uri.path.clone()
        } else {
            format!("{}/", uri.path)
        };
        let new_path = format!("{base}{relative}");
        VsUri::from_components(&uri.scheme, &uri.authority, &new_path, &uri.query, &uri.fragment)
    }

    /// Remove dot segments (`.` and `..`) from a path string per RFC 3986 §5.2.4.
    pub fn remove_dot_segments(path: &str) -> String {
        let mut output_segments: Vec<&str> = Vec::new();
        for segment in path.split('/') {
            match segment {
                "." => {}
                ".." => {
                    output_segments.pop();
                }
                s => output_segments.push(s),
            }
        }
        let result = output_segments.join("/");
        if path.starts_with('/') && !result.starts_with('/') {
            format!("/{result}")
        } else {
            result
        }
    }

    /// Return the depth (number of non-empty segments) of a URI path.
    pub fn depth(uri: &VsUri) -> usize {
        uri.path.split('/').filter(|s| !s.is_empty()).count()
    }

    /// Return true if `child`'s path is a descendant of `parent`'s path
    /// (same scheme and authority assumed).
    pub fn is_child_of(parent: &VsUri, child: &VsUri) -> bool {
        if parent.scheme != child.scheme || parent.authority != child.authority {
            return false;
        }
        let p = if parent.path.ends_with('/') {
            parent.path.clone()
        } else {
            format!("{}/", parent.path)
        };
        child.path.starts_with(&p) && child.path.len() > p.len()
    }

    /// Return the common path prefix shared by two URIs.
    pub fn common_prefix(a: &VsUri, b: &VsUri) -> String {
        let segs_a: Vec<&str> = a.path.split('/').collect();
        let segs_b: Vec<&str> = b.path.split('/').collect();
        let mut common = Vec::new();
        for (sa, sb) in segs_a.iter().zip(segs_b.iter()) {
            if sa == sb {
                common.push(*sa);
            } else {
                break;
            }
        }
        common.join("/")
    }
}

// ---------------------------------------------------------------------------
// UriSchemeRegistry – dynamic scheme validation / metadata
// ---------------------------------------------------------------------------

/// A registry that tracks known URI schemes and their properties.
#[derive(Debug, Clone)]
pub struct UriSchemeRegistry {
    schemes: HashMap<String, UriSchemeInfo>,
}

/// Metadata about a URI scheme.
#[derive(Debug, Clone)]
pub struct UriSchemeInfo {
    pub name: String,
    pub default_port: Option<u16>,
    pub is_secure: bool,
    pub description: String,
}

impl UriSchemeRegistry {
    /// Create a registry pre-populated with common schemes.
    pub fn with_defaults() -> Self {
        let mut reg = Self {
            schemes: HashMap::new(),
        };
        reg.register(UriSchemeInfo {
            name: "http".into(),
            default_port: Some(80),
            is_secure: false,
            description: "Hypertext Transfer Protocol".into(),
        });
        reg.register(UriSchemeInfo {
            name: "https".into(),
            default_port: Some(443),
            is_secure: true,
            description: "HTTP Secure".into(),
        });
        reg.register(UriSchemeInfo {
            name: "ftp".into(),
            default_port: Some(21),
            is_secure: false,
            description: "File Transfer Protocol".into(),
        });
        reg.register(UriSchemeInfo {
            name: "ssh".into(),
            default_port: Some(22),
            is_secure: true,
            description: "Secure Shell".into(),
        });
        reg.register(UriSchemeInfo {
            name: "file".into(),
            default_port: None,
            is_secure: false,
            description: "Local file system".into(),
        });
        reg
    }

    /// Register a new scheme (or update an existing one).
    pub fn register(&mut self, info: UriSchemeInfo) {
        self.schemes.insert(info.name.to_lowercase(), info);
    }

    /// Look up scheme info by name.
    pub fn get(&self, scheme: &str) -> Option<&UriSchemeInfo> {
        self.schemes.get(&scheme.to_lowercase())
    }

    /// Check whether a scheme is registered.
    pub fn is_known(&self, scheme: &str) -> bool {
        self.schemes.contains_key(&scheme.to_lowercase())
    }

    /// Return the default port for a scheme, if any.
    pub fn default_port(&self, scheme: &str) -> Option<u16> {
        self.get(scheme).and_then(|i| i.default_port)
    }

    /// Check whether a scheme is considered secure.
    pub fn is_secure(&self, scheme: &str) -> bool {
        self.get(scheme).is_some_and(|i| i.is_secure)
    }

    /// Return the number of registered schemes.
    pub fn len(&self) -> usize {
        self.schemes.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.schemes.is_empty()
    }

    /// List all registered scheme names.
    pub fn scheme_names(&self) -> Vec<&str> {
        self.schemes.keys().map(|s| s.as_str()).collect()
    }
}

impl fmt::Display for UriSchemeRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UriSchemeRegistry({} schemes)", self.len())
    }
}



// ---------------------------------------------------------------------------
// uri – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for URI parsing and manipulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YUriUriScheme {
    File,
    Http,
    Https,
    Custom,
}

impl YUriUriScheme {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::File => 0,
            Self::Http => 1,
            Self::Https => 2,
            Self::Custom => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Http => "Http",
            Self::Https => "Https",
            Self::Custom => "Custom",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YUriUriScheme] {
        &[
            YUriUriScheme::File,
            YUriUriScheme::Http,
            YUriUriScheme::Https,
            YUriUriScheme::Custom,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YUriUriScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks URI components data.
#[derive(Debug, Clone)]
pub struct YUriUriComponents {
    pub scheme: String,
    pub authority: String,
    pub path: String,
}

impl YUriUriComponents {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            scheme: String::new(),
            authority: String::new(),
            path: String::new(),
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YUriUriComponents({}: {:?})", "scheme", self.scheme)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_uri_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_uri_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_uri_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_uri_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_uri_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_uri_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_uri_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_uri_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// uri – Extended URI template expander helpers
// ---------------------------------------------------------------------------

/// Priority levels for URI template expander.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZUriPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZUriPriority {
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
    pub fn all_asc() -> [ZUriPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZUriPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks URI template expander data.
#[derive(Debug, Clone)]
pub struct ZUriUriTemplateExpander {
    pub variables: Vec<(String, String)>,
    pub template: String,
    pub strict: bool,
}

impl ZUriUriTemplateExpander {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
            template: String::new(),
            strict: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.variables.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZUriUriTemplateExpander[template={:?}, strict={:?}]", self.template, self.strict)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.strict = !c.strict;
        c
    }
}

/// Compute a simple rolling hash for URI template expander.
pub fn z_uri_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_uri_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_uri_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_uri_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_uri_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_uri_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_uri_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 93
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer93 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer93 {
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
pub fn xb_fnv1a_93(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_93<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_93<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_93(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_93(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 192
// ---------------------------------------------------------------------------

/// Generic object pool `Xc192Pool<T>`.
pub struct Xc192Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc192Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc192PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc192Pool<T> {
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
    pub fn stats(&self) -> Xc192PoolStats {
        Xc192PoolStats {
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

impl<T> Default for Xc192Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc192Scheduler`.
pub struct Xc192Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc192Scheduler {
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

impl Default for Xc192Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_192 hash for the given byte slice.
pub fn xc_192_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_192 convention.
pub fn xc_192_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe106 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe106Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe106PipelineError {
    pub stage: Xe106Stage,
    pub message: String,
}

impl std::fmt::Display for Xe106PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe106Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe106Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError>>>,
    stage_names: Vec<Xe106Stage>,
}

impl Xe106Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe106Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe106Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe106Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe106Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> {
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

    pub fn compose(mut self, other: Xe106Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe106CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe106CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe106Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe106CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe106CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe106Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe106CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_106_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe106CacheEntry {
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

    fn xe_106_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe106CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_106_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> {
    Ok(data)
}

pub fn xe_106_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_106_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_106_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_106_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe106PipelineError> {
    Err(Xe106PipelineError {
        stage: Xe106Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_104: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg104Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg104Graph {
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

impl Default for Xg104Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_104: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg104Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg104Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg104Heap<T>) {
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

impl<T: Ord> Default for Xg104Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 191).
pub struct Xh191SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh191SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 233 as u64,
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

/// A compact bit set supporting boolean operations (variant 191).
pub struct Xh191BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh191BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 191).
pub struct Xi191Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi191Deque<T> {
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
pub struct Xi191Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi191Interval {
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

/// A simple interval tree (variant 191).
pub struct Xi191IntervalTree {
    xi_intervals: Vec<Xi191Interval>,
}

impl Xi191IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi191Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi191Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi191Interval) -> Vec<&Xi191Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi191Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi191Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi191Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi191Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi191Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi191Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 192) ---

/// Disjoint set / union-find for crate 192.
pub struct Xj192UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj192UnionFind {
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

const XJ192_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 192.
pub struct Xj192BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj192BTreeNode<K, V>>>,
    len: usize,
}

struct Xj192BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj192BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj192BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ192_BTREE_ORDER - 1
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
        let mid = XJ192_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj192BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj192BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj192BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj192BTreeNode::xj_new_leaf();
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


// --- xk_191 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk191SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk191SegmentTree {
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
pub struct Xk191DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk191DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_192).
#[derive(Debug, Clone)]
pub struct Xl192Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl192Rope {
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

/// Suffix array for efficient string searching (xl_192).
#[derive(Debug, Clone)]
pub struct Xl192SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl192SuffixArray {
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

    // -- from_components ----------------------------------------------------

    #[test]
    fn from_components_basic() {
        let uri = VsUri::from_components("https", "example.com", "/path", "q=1", "frag");
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com");
        assert_eq!(uri.path, "/path");
        assert_eq!(uri.query, "q=1");
        assert_eq!(uri.fragment, "frag");
    }

    #[test]
    fn from_components_empty() {
        let uri = VsUri::from_components("", "", "", "", "");
        assert_eq!(uri.scheme, "");
        assert_eq!(uri.to_uri_string(), "");
    }

    #[test]
    fn from_components_normalizes_backslashes() {
        let uri = VsUri::from_components("file", "", "C:\\Users\\foo", "", "");
        assert_eq!(uri.path, "C:/Users/foo");
    }

    // -- file ---------------------------------------------------------------

    #[test]
    fn file_unix_path() {
        let uri = VsUri::file("/home/user/file.rs");
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.authority, "");
        assert_eq!(uri.path, "/home/user/file.rs");
    }

    #[test]
    fn file_windows_path() {
        let uri = VsUri::file("C:\\Users\\foo\\bar.txt");
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.path, "/c:/Users/foo/bar.txt");
    }

    #[test]
    fn file_windows_lowercase_drive() {
        let uri = VsUri::file("D:\\Projects\\test");
        assert!(uri.path.starts_with("/d:"));
    }

    #[test]
    fn file_already_forward_slashes() {
        let uri = VsUri::file("/already/forward/slashes");
        assert_eq!(uri.path, "/already/forward/slashes");
    }

    // -- parse --------------------------------------------------------------

    #[test]
    fn parse_https_full() {
        let uri = VsUri::parse("https://example.com/path?q=1#frag");
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com");
        assert_eq!(uri.path, "/path");
        assert_eq!(uri.query, "q=1");
        assert_eq!(uri.fragment, "frag");
    }

    #[test]
    fn parse_file_uri() {
        let uri = VsUri::parse("file:///home/user/file.rs");
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.authority, "");
        assert_eq!(uri.path, "/home/user/file.rs");
    }

    #[test]
    fn parse_file_uri_windows() {
        let uri = VsUri::parse("file:///c%3A/Users/foo");
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.path, "/c:/Users/foo");
    }

    #[test]
    fn parse_opaque_uri() {
        let uri = VsUri::parse("untitled:Untitled-1");
        assert_eq!(uri.scheme, "untitled");
        assert_eq!(uri.path, "Untitled-1");
    }

    #[test]
    fn parse_command_uri() {
        let uri = VsUri::parse("command:editor.action.formatDocument");
        assert_eq!(uri.scheme, "command");
        assert_eq!(uri.path, "editor.action.formatDocument");
    }

    #[test]
    fn parse_empty_string() {
        let uri = VsUri::parse("");
        assert_eq!(uri.scheme, "");
        assert_eq!(uri.path, "");
    }

    #[test]
    fn parse_http_with_port() {
        let uri = VsUri::parse("http://localhost:8080/index.html");
        assert_eq!(uri.scheme, "http");
        assert_eq!(uri.authority, "localhost:8080");
        assert_eq!(uri.path, "/index.html");
    }

    #[test]
    fn parse_mailto() {
        let uri = VsUri::parse("mailto:user@example.com");
        assert_eq!(uri.scheme, "mailto");
    }

    #[test]
    fn parse_vscode_remote() {
        let uri = VsUri::parse("vscode-remote://ssh-remote+myhost/home/user/project");
        assert_eq!(uri.scheme, "vscode-remote");
        assert_eq!(uri.authority, "ssh-remote+myhost");
        assert_eq!(uri.path, "/home/user/project");
    }

    // -- with ---------------------------------------------------------------

    #[test]
    fn with_change_path() {
        let base = VsUri::parse("https://example.com/old?q=1");
        let changed = VsUri::with(
            &base,
            UriChanges {
                path: Some("/new".into()),
                ..Default::default()
            },
        );
        assert_eq!(changed.path, "/new");
        assert_eq!(changed.scheme, "https");
        assert_eq!(changed.authority, "example.com");
        assert_eq!(changed.query, "q=1");
    }

    #[test]
    fn with_change_scheme() {
        let base = VsUri::file("/tmp/a.txt");
        let changed = VsUri::with(
            &base,
            UriChanges {
                scheme: Some("untitled".into()),
                ..Default::default()
            },
        );
        assert_eq!(changed.scheme, "untitled");
        assert_eq!(changed.path, "/tmp/a.txt");
    }

    #[test]
    fn with_change_fragment() {
        let base = VsUri::parse("https://example.com/page");
        let changed = VsUri::with(
            &base,
            UriChanges {
                fragment: Some("section".into()),
                ..Default::default()
            },
        );
        assert_eq!(changed.fragment, "section");
    }

    #[test]
    fn with_no_changes() {
        let base = VsUri::parse("https://example.com/path?q=1#frag");
        let same = VsUri::with(&base, UriChanges::default());
        assert_eq!(base, same);
    }

    // -- is_file / is_untitled ----------------------------------------------

    #[test]
    fn is_file_true() {
        assert!(VsUri::file("/tmp/a.txt").is_file());
    }

    #[test]
    fn is_file_false() {
        assert!(!VsUri::parse("https://example.com").is_file());
    }

    #[test]
    fn is_untitled_true() {
        assert!(VsUri::parse("untitled:Untitled-1").is_untitled());
    }

    #[test]
    fn is_untitled_false() {
        assert!(!VsUri::file("/tmp/a.txt").is_untitled());
    }

    // -- fs_path ------------------------------------------------------------

    #[test]
    fn fs_path_unix() {
        let uri = VsUri::file("/home/user/file.rs");
        assert_eq!(uri.fs_path(), "/home/user/file.rs");
    }

    #[test]
    fn fs_path_roundtrip() {
        let original = "/home/user/some path/file.rs";
        let uri = VsUri::file(original);
        assert_eq!(uri.fs_path(), original);
    }

    // -- to_string / Display ------------------------------------------------

    #[test]
    fn display_file_uri() {
        let uri = VsUri::file("/home/user/file.rs");
        let s = uri.to_string();
        assert!(s.starts_with("file:///"));
        assert!(s.contains("/home/user/file.rs"));
    }

    #[test]
    fn display_https_uri() {
        let uri = VsUri::parse("https://example.com/path?q=1#frag");
        let s = uri.to_string();
        assert!(s.starts_with("https://"));
        assert!(s.contains("example.com"));
        assert!(s.contains("/path"));
        assert!(s.contains("?q"));
        assert!(s.contains("#frag"));
    }

    #[test]
    fn display_opaque_uri() {
        let uri = VsUri::parse("command:workbench.action.openSettings");
        let s = uri.to_string();
        assert_eq!(s, "command:workbench.action.openSettings");
    }

    // -- encoding -----------------------------------------------------------

    #[test]
    fn encode_space_in_path() {
        let uri = VsUri::file("/home/user/my file.txt");
        let s = uri.to_string();
        assert!(s.contains("my%20file.txt"));
    }

    #[test]
    fn encode_special_chars() {
        let uri = VsUri::from_components("https", "example.com", "/path/café", "", "");
        let s = uri.to_string();
        assert!(s.contains("caf%C3%A9"));
    }

    #[test]
    fn decode_percent_encoded_path() {
        let uri = VsUri::parse("file:///home/user/my%20file.txt");
        assert_eq!(uri.path, "/home/user/my file.txt");
    }

    // -- Windows path handling (cross-platform logic test) -------------------

    #[test]
    fn uri_path_to_fs_path_windows_drive() {
        let result = uri_path_to_fs_path("/c:/Users/foo/bar.txt", true);
        assert_eq!(result, "C:\\Users\\foo\\bar.txt");
    }

    #[test]
    fn uri_path_to_fs_path_unix() {
        let result = uri_path_to_fs_path("/home/user/file.rs", false);
        assert_eq!(result, "/home/user/file.rs");
    }

    #[test]
    fn uri_path_to_fs_path_windows_unc() {
        let result = uri_path_to_fs_path("/server/share/file.txt", true);
        assert_eq!(result, "server\\share\\file.txt");
    }

    // -- serde roundtrip ----------------------------------------------------

    #[test]
    fn serde_serialize() {
        let uri = VsUri::file("/home/user/file.rs");
        let json = serde_json::to_string(&uri).unwrap();
        assert!(json.starts_with('"'));
        assert!(json.contains("file:///home/user/file.rs"));
    }

    #[test]
    fn serde_roundtrip() {
        let uri = VsUri::parse("https://example.com/path?q=1#frag");
        let json = serde_json::to_string(&uri).unwrap();
        let back: VsUri = serde_json::from_str(&json).unwrap();
        assert_eq!(uri, back);
    }

    #[test]
    fn serde_roundtrip_file() {
        let uri = VsUri::file("/tmp/test.txt");
        let json = serde_json::to_string(&uri).unwrap();
        let back: VsUri = serde_json::from_str(&json).unwrap();
        assert_eq!(uri.scheme, back.scheme);
        assert_eq!(uri.path, back.path);
    }

    // -- equality / ordering ------------------------------------------------

    #[test]
    fn equality() {
        let a = VsUri::file("/home/user/file.rs");
        let b = VsUri::file("/home/user/file.rs");
        assert_eq!(a, b);
    }

    #[test]
    fn inequality_different_path() {
        let a = VsUri::file("/home/user/a.rs");
        let b = VsUri::file("/home/user/b.rs");
        assert_ne!(a, b);
    }

    #[test]
    fn ordering() {
        let a = VsUri::file("/a");
        let b = VsUri::file("/b");
        assert!(a < b);
    }

    // -- hash ---------------------------------------------------------------

    #[test]
    fn hash_consistency() {
        use std::collections::HashSet;
        let a = VsUri::file("/home/user/file.rs");
        let b = VsUri::file("/home/user/file.rs");
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    // -- clone --------------------------------------------------------------

    #[test]
    fn clone_independence() {
        let a = VsUri::file("/tmp/test.txt");
        let b = a.clone();
        assert_eq!(a, b);
    }

    // -- constants ----------------------------------------------------------

    #[test]
    fn scheme_constants() {
        assert_eq!(FILE, "file");
        assert_eq!(UNTITLED, "untitled");
        assert_eq!(VSCODE, "vscode");
        assert_eq!(VSCODE_REMOTE, "vscode-remote");
        assert_eq!(HTTP, "http");
        assert_eq!(HTTPS, "https");
        assert_eq!(MAILTO, "mailto");
        assert_eq!(DATA, "data");
        assert_eq!(COMMAND, "command");
        assert_eq!(VSCODE_SETTINGS, "vscode-settings");
        assert_eq!(VSCODE_USERDATA, "vscode-userdata");
    }

    // -- edge cases ---------------------------------------------------------

    #[test]
    fn parse_data_uri() {
        let uri = VsUri::parse("data:text/plain;base64,SGVsbG8=");
        assert_eq!(uri.scheme, "data");
    }

    #[test]
    fn parse_query_only() {
        let uri = VsUri::parse("https://example.com?q=hello+world");
        assert_eq!(uri.scheme, "https");
        assert!(!uri.query.is_empty());
    }

    #[test]
    fn parse_fragment_only() {
        let uri = VsUri::parse("https://example.com#section");
        assert_eq!(uri.fragment, "section");
    }

    #[test]
    fn file_path_with_hash() {
        let uri = VsUri::file("/tmp/file#1.txt");
        assert_eq!(uri.path, "/tmp/file#1.txt");
        let s = uri.to_string();
        assert!(!s.contains("#1.txt"));
        assert!(s.contains("%231.txt"));
    }

    #[test]
    fn percent_decode_multibyte() {
        let decoded = percent_decode("caf%C3%A9");
        assert_eq!(decoded, "café");
    }

    #[test]
    fn percent_decode_plain() {
        let decoded = percent_decode("hello");
        assert_eq!(decoded, "hello");
    }

    #[test]
    fn lowercase_drive_letter_fn() {
        assert_eq!(lowercase_drive_letter("/C:/foo"), "/c:/foo");
        assert_eq!(lowercase_drive_letter("/c:/foo"), "/c:/foo");
        assert_eq!(lowercase_drive_letter("/home/user"), "/home/user");
    }

    // ---- URI resolution tests ----

    #[test]
    fn resolve_relative_sibling() {
        let base = VsUri::parse("https://example.com/a/b/c");
        let resolved = base.resolve("d");
        assert_eq!(resolved.path, "/a/b/d");
        assert_eq!(resolved.scheme, "https");
        assert_eq!(resolved.authority, "example.com");
    }

    #[test]
    fn resolve_relative_dotdot() {
        let base = VsUri::parse("https://example.com/a/b/c");
        let resolved = base.resolve("../d");
        assert_eq!(resolved.path, "/a/d");
    }

    #[test]
    fn resolve_absolute_path() {
        let base = VsUri::parse("https://example.com/a/b/c");
        let resolved = base.resolve("/x/y");
        assert_eq!(resolved.path, "/x/y");
        assert_eq!(resolved.authority, "example.com");
    }

    #[test]
    fn resolve_empty_returns_self() {
        let base = VsUri::parse("https://example.com/a/b/c");
        let resolved = base.resolve("");
        assert_eq!(resolved, base);
    }

    #[test]
    fn resolve_full_uri() {
        let base = VsUri::parse("https://example.com/a/b/c");
        let resolved = base.resolve("https://other.com/x");
        assert_eq!(resolved.scheme, "https");
        assert_eq!(resolved.authority, "other.com");
        assert_eq!(resolved.path, "/x");
    }

    #[test]
    fn resolve_with_query_and_fragment() {
        let base = VsUri::parse("https://example.com/a/b");
        let resolved = base.resolve("c?key=val#section");
        assert_eq!(resolved.path, "/a/c");
        assert_eq!(resolved.query, "key=val");
        assert_eq!(resolved.fragment, "section");
    }

    // ---- QueryParams tests ----

    #[test]
    fn query_params_parse_basic() {
        let params = QueryParams::parse("foo=bar&baz=qux");
        assert_eq!(params.len(), 2);
        assert_eq!(params.get("foo"), Some("bar"));
        assert_eq!(params.get("baz"), Some("qux"));
        assert!(params.has("foo"));
        assert!(!params.has("missing"));
    }

    #[test]
    fn query_params_parse_empty() {
        let params = QueryParams::parse("");
        assert!(params.is_empty());
    }

    #[test]
    fn query_params_no_value() {
        let params = QueryParams::parse("flag");
        assert_eq!(params.len(), 1);
        assert_eq!(params.get("flag"), Some(""));
    }

    #[test]
    fn query_params_duplicate_keys() {
        let params = QueryParams::parse("a=1&a=2&a=3");
        assert_eq!(params.get("a"), Some("1")); // first
        assert_eq!(params.get_all("a"), vec!["1", "2", "3"]);
    }

    #[test]
    fn query_params_build_roundtrip() {
        let mut params = QueryParams::parse("");
        params.set("name", "hello world");
        params.set("page", "2");
        let qs = params.to_query_string();
        assert!(qs.contains("name="));
        assert!(qs.contains("page=2"));
    }

    #[test]
    fn query_params_remove() {
        let mut params = QueryParams::parse("a=1&b=2&a=3");
        assert_eq!(params.remove("a"), 2);
        assert_eq!(params.len(), 1);
        assert!(!params.has("a"));
    }

    #[test]
    fn query_params_from_uri() {
        let uri = VsUri::parse("https://example.com/path?key=value&other=123");
        let params = QueryParams::from_uri(&uri);
        assert_eq!(params.get("key"), Some("value"));
        assert_eq!(params.get("other"), Some("123"));
    }

    #[test]
    fn query_params_keys() {
        let params = QueryParams::parse("x=1&y=2&z=3");
        assert_eq!(params.keys(), vec!["x", "y", "z"]);
    }

    // ---- DataUri tests ----

    #[test]
    fn data_uri_decode_plain() {
        let data = DataUri::decode("data:text/plain,Hello%20World").unwrap();
        assert_eq!(data.mime_type, "text/plain");
        assert_eq!(data.data, b"Hello World");
        assert!(!data.is_base64);
    }

    #[test]
    fn data_uri_decode_base64() {
        let data = DataUri::decode("data:text/plain;base64,SGVsbG8=").unwrap();
        assert_eq!(data.mime_type, "text/plain");
        assert_eq!(data.data, b"Hello");
        assert!(data.is_base64);
    }

    #[test]
    fn data_uri_decode_invalid() {
        assert!(DataUri::decode("not-a-data-uri").is_none());
        assert!(DataUri::decode("data:text/plain").is_none()); // no comma
    }

    #[test]
    fn data_uri_encode_roundtrip_base64() {
        let original = DataUri {
            mime_type: "application/octet-stream".to_string(),
            data: vec![0, 1, 2, 3, 255],
            is_base64: true,
        };
        let encoded = original.encode();
        let decoded = DataUri::decode(&encoded).unwrap();
        assert_eq!(decoded.data, original.data);
        assert_eq!(decoded.mime_type, original.mime_type);
    }

    #[test]
    fn data_uri_encode_plain() {
        let original = DataUri {
            mime_type: "text/plain".to_string(),
            data: b"Hello".to_vec(),
            is_base64: false,
        };
        let encoded = original.encode();
        assert!(encoded.starts_with("data:text/plain,"));
    }

    #[test]
    fn normalize_path_removes_dots() {
        assert_eq!(normalize_path("/a/b/../c"), "/a/c");
        assert_eq!(normalize_path("/a/./b"), "/a/b");
        assert_eq!(normalize_path("/a/b/../../c"), "/c");
    }

    // ---- UriBuilder tests ----

    #[test]
    fn uri_builder_full() {
        let uri = UriBuilder::new()
            .scheme("https")
            .authority("example.com:8080")
            .path("/api/v1/users")
            .query("page=1&limit=10")
            .fragment("top")
            .build();
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com:8080");
        assert_eq!(uri.path, "/api/v1/users");
        assert_eq!(uri.query, "page=1&limit=10");
        assert_eq!(uri.fragment, "top");
        assert_eq!(
            uri.to_string(),
            "https://example.com:8080/api/v1/users?page=1&limit=10#top"
        );
    }

    #[test]
    fn uri_builder_file_shorthand() {
        let uri = UriBuilder::new().scheme("file").path("/home/user/file.rs").build();
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.path, "/home/user/file.rs");
        assert!(uri.authority.is_empty());
    }

    #[test]
    fn uri_builder_with_query_params() {
        let mut params = QueryParams::parse("");
        params.set("search", "rust uri");
        params.set("page", "3");
        let uri = UriBuilder::new()
            .scheme("https")
            .authority("example.com")
            .path("/search")
            .query_params(&params)
            .build();
        assert!(uri.query.contains("search="));
        assert!(uri.query.contains("page=3"));
    }

    // ---- UriMatcher tests ----

    #[test]
    fn uri_matcher_exact() {
        let m = UriMatcher::new("https://example.com/api/v1/users");
        assert!(m.matches(&VsUri::parse("https://example.com/api/v1/users")));
        assert!(!m.matches(&VsUri::parse("https://example.com/api/v1/posts")));
    }

    #[test]
    fn uri_matcher_glob_star() {
        let m = UriMatcher::new("https://example.com/api/**");
        assert!(m.matches(&VsUri::parse("https://example.com/api/v1/users")));
        assert!(m.matches(&VsUri::parse("https://example.com/api/v2/posts/123")));
        assert!(!m.matches(&VsUri::parse("https://example.com/other")));
    }

    #[test]
    fn uri_matcher_wildcard_scheme() {
        let m = UriMatcher::new("*://example.com/path");
        assert!(m.matches(&VsUri::parse("https://example.com/path")));
        assert!(m.matches(&VsUri::parse("http://example.com/path")));
        assert!(!m.matches(&VsUri::parse("https://other.com/path")));
    }

    #[test]
    fn uri_matcher_single_star_segment() {
        let m = UriMatcher::new("file:///home/*/projects");
        assert!(m.matches(&VsUri::parse("file:///home/alice/projects")));
        assert!(m.matches(&VsUri::parse("file:///home/bob/projects")));
        assert!(!m.matches(&VsUri::parse("file:///home/alice/bob/projects")));
    }

    // ---- resolve_relative free function ----

    #[test]
    fn resolve_relative_fn_basic() {
        let base = VsUri::parse("https://example.com/a/b/c");
        let resolved = resolve_relative(&base, "../d?q=1#frag");
        assert_eq!(resolved.path, "/a/d");
        assert_eq!(resolved.query, "q=1");
        assert_eq!(resolved.fragment, "frag");
    }

    // ---- From / Display impls ----

    #[test]
    fn from_string_for_vsuri() {
        let uri: VsUri = "https://example.com/path".into();
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com");
    }

    #[test]
    fn from_vsuri_for_string() {
        let uri = VsUri::parse("https://example.com/path");
        let s: String = uri.into();
        assert!(s.starts_with("https://example.com"));
    }

    #[test]
    fn query_params_display() {
        let params = QueryParams::parse("a=1&b=2");
        let s = params.to_string();
        assert!(s.contains("a=1"));
        assert!(s.contains("b=2"));
    }

    #[test]
    fn query_params_iter() {
        let params = QueryParams::parse("x=1&y=2&z=3");
        let collected: Vec<_> = params.iter().collect();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0], ("x", "1"));
        assert_eq!(collected[2], ("z", "3"));
    }

    // -- uri_extension ---------------------------------------------------------

    #[test]
    fn uri_extension_rs() {
        let uri = VsUri::file("/src/main.rs");
        assert_eq!(uri_extension(&uri), Some("rs"));
    }

    #[test]
    fn uri_extension_none() {
        let uri = VsUri::file("/Makefile");
        assert_eq!(uri_extension(&uri), None);
    }

    // -- uri_filename ----------------------------------------------------------

    #[test]
    fn uri_filename_extracts() {
        let uri = VsUri::file("/home/user/doc.txt");
        assert_eq!(uri_filename(&uri), "doc.txt");
    }

    // -- uri_parent ------------------------------------------------------------

    #[test]
    fn uri_parent_extracts() {
        let uri = VsUri::file("/home/user/doc.txt");
        assert_eq!(uri_parent(&uri), "/home/user");
    }

    // -- path_segments ---------------------------------------------------------

    #[test]
    fn path_segments_splits() {
        let uri = VsUri::file("/a/b/c");
        assert_eq!(path_segments(&uri), vec!["a", "b", "c"]);
    }

    // -- same_origin -----------------------------------------------------------

    #[test]
    fn same_origin_true() {
        let a = VsUri::from_components("https", "example.com", "/a", "", "");
        let b = VsUri::from_components("https", "example.com", "/b", "", "");
        assert!(same_origin(&a, &b));
    }

    #[test]
    fn same_origin_false() {
        let a = VsUri::from_components("https", "example.com", "/a", "", "");
        let b = VsUri::from_components("http", "example.com", "/b", "", "");
        assert!(!same_origin(&a, &b));
    }

    // -- pct_decode -------------------------------------------------------------

    #[test]
    fn pct_decode_spaces() {
        assert_eq!(pct_decode("hello%20world"), "hello world");
    }

    #[test]
    fn pct_decode_plain() {
        assert_eq!(pct_decode("abc"), "abc");
    }

    // -- UriTemplate -------------------------------------------------------

    #[test]
    fn uri_template_expand() {
        let t = UriTemplate::new("https://{host}/api/{version}");
        let result = t.expand(&[("host", "example.com"), ("version", "v2")]);
        assert_eq!(result, "https://example.com/api/v2");
    }

    #[test]
    fn uri_template_variable_names() {
        let t = UriTemplate::new("{scheme}://{host}/{path}");
        let names = t.variable_names();
        assert_eq!(names, vec!["scheme", "host", "path"]);
    }

    #[test]
    fn uri_template_fully_expanded() {
        let t = UriTemplate::new("{a}/{b}");
        assert!(!t.is_fully_expanded(&[("a", "1")]));
        assert!(t.is_fully_expanded(&[("a", "1"), ("b", "2")]));
    }

    #[test]
    fn uri_template_display() {
        let t = UriTemplate::new("http://{host}");
        let s = format!("{t}");
        assert!(s.contains("UriTemplate"));
    }

    // -- UriNormalizer -----------------------------------------------------

    #[test]
    fn uri_normalizer_lowercase() {
        let uri = VsUri::from_components("HTTPS", "Example.COM", "/path/", "", "");
        let normalized = UriNormalizer::normalize(&uri);
        assert_eq!(normalized.scheme, "https");
        assert_eq!(normalized.authority, "example.com");
    }

    #[test]
    fn uri_normalizer_strip_default_port() {
        let uri = VsUri::from_components("http", "example.com:80", "/", "", "");
        let normalized = UriNormalizer::normalize(&uri);
        assert_eq!(normalized.authority, "example.com");
    }

    #[test]
    fn uri_normalizer_equivalence() {
        let a = VsUri::from_components("HTTP", "Example.com:80", "/path", "", "");
        let b = VsUri::from_components("http", "example.com", "/path", "", "");
        assert!(UriNormalizer::are_equivalent(&a, &b));
    }

    // -- UriQueryBuilder ---------------------------------------------------

    #[test]
    fn query_builder_basic() {
        let q = UriQueryBuilder::new()
            .param("key", "value")
            .param("foo", "bar");
        assert_eq!(q.build(), "key=value&foo=bar");
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn query_builder_encoding() {
        let q = UriQueryBuilder::new().param("q", "hello world");
        assert_eq!(q.build(), "q=hello+world");
    }

    #[test]
    fn query_builder_display() {
        let q = UriQueryBuilder::new().param("a", "1");
        let s = format!("{q}");
        assert!(s.starts_with('?'));
    }

    // -- UriAuthority ------------------------------------------------------

    #[test]
    fn authority_parse_simple() {
        let a = UriAuthority::parse("example.com:8080");
        assert_eq!(a.host, "example.com");
        assert_eq!(a.port, Some(8080));
        assert!(!a.has_userinfo());
    }

    #[test]
    fn authority_parse_with_userinfo() {
        let a = UriAuthority::parse("user:pass@example.com:443");
        assert_eq!(a.userinfo, Some("user:pass".to_string()));
        assert_eq!(a.host, "example.com");
        assert_eq!(a.port, Some(443));
    }

    #[test]
    fn authority_parse_no_port() {
        let a = UriAuthority::parse("example.com");
        assert_eq!(a.host, "example.com");
        assert_eq!(a.port, None);
    }

    #[test]
    fn authority_display() {
        let a = UriAuthority::parse("user@host:80");
        let s = format!("{a}");
        assert_eq!(s, "user@host:80");
    }

    #[test] fn uriEncodeDecoder_new() { let s = UriEncodeDecoder::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn uriEncodeDecoder_add() { let mut s = UriEncodeDecoder::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn uriEncodeDecoder_remove() { let mut s = UriEncodeDecoder::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn uriEncodeDecoder_config() { let mut s = UriEncodeDecoder::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn uriEncodeDecoder_nav() { let mut s = UriEncodeDecoder::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn uriEncodeDecoder_filter() { let mut s = UriEncodeDecoder::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn uriEncodeDecoder_display() { assert!(format!("{}", UriEncodeDecoder::new()).contains("UriEncodeDecoder")); }
    #[test] fn uriCanonicalComparator_new() { let s = UriCanonicalComparator::new(); assert!(s.is_empty()); }
    #[test] fn uriCanonicalComparator_add() { let mut s = UriCanonicalComparator::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn uriCanonicalComparator_active() { let mut s = UriCanonicalComparator::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn uriCanonicalComparator_error() { let mut s = UriCanonicalComparator::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn uriCanonicalComparator_rm_group() { let mut s = UriCanonicalComparator::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn uriCanonicalComparator_display() { assert!(format!("{}", UriCanonicalComparator::new()).contains("UriCanonicalComparator")); }


    #[test] fn uriEncodeDecoder_snap_capture() {
        let s = UriEncodeDecoder::new();
        let snap = UriEncodeDecoderSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn uriEncodeDecoder_snap_stale() {
        let s = UriEncodeDecoder::new();
        let snap = UriEncodeDecoderSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn uriEncodeDecoder_snap_diff() {
        let s = UriEncodeDecoder::new();
        let s1v = UriEncodeDecoderSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn uriEncodeDecoder_snap_display() {
        let s = UriEncodeDecoder::new();
        let snap = UriEncodeDecoderSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn uriCanonicalComparator_stats_record() {
        let mut st = UriCanonicalComparatorStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn uriCanonicalComparator_stats_hit_ratio() {
        let mut st = UriCanonicalComparatorStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn uriCanonicalComparator_stats_merge() {
        let mut a = UriCanonicalComparatorStats::new();
        a.total_adds = 5;
        let mut b = UriCanonicalComparatorStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn uriCanonicalComparator_stats_display() {
        let st = UriCanonicalComparatorStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn uriEncodeDecoder_config_default() {
        let c = UriEncodeDecoderConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn uriEncodeDecoder_config_builder() {
        let c = UriEncodeDecoderConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn uriEncodeDecoder_config_labels() {
        let mut c = UriEncodeDecoderConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn uriEncodeDecoder_config_cleanup_threshold() {
        let c = UriEncodeDecoderConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn uriEncodeDecoder_config_display() {
        assert!(format!("{}", UriEncodeDecoderConfig::new()).contains("Config"));
    }
    #[test] fn uriCanonicalComparator_stats_peaks() {
        let mut st = UriCanonicalComparatorStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- UriTemplateExpander ------------------------------------------------

    #[test]
    fn template_expander_basic_expansion() {
        let mut exp = UriTemplateExpander::new("https://{host}/api/{version}/users");
        exp.register_variable("host", "example.com");
        exp.register_variable("version", "v2");
        assert_eq!(exp.expand(), "https://example.com/api/v2/users");
    }

    #[test]
    fn template_expander_unresolved_vars() {
        let mut exp = UriTemplateExpander::new("{scheme}://{host}/{path}");
        exp.register_variable("scheme", "https");
        let unresolved = exp.unresolved_vars();
        assert_eq!(unresolved.len(), 2);
        assert!(unresolved.contains(&"host".to_string()));
        assert!(unresolved.contains(&"path".to_string()));
        assert!(!exp.is_fully_resolved());
    }

    #[test]
    fn template_expander_fully_resolved() {
        let mut exp = UriTemplateExpander::new("{a}/{b}");
        exp.register_all(&[("a", "x"), ("b", "y")]);
        assert!(exp.is_fully_resolved());
        assert_eq!(exp.expand(), "x/y");
    }

    #[test]
    fn template_expander_unregister_and_clear() {
        let mut exp = UriTemplateExpander::new("{a}/{b}");
        exp.register_all(&[("a", "1"), ("b", "2")]);
        assert_eq!(exp.variable_count(), 2);
        assert!(exp.unregister("a"));
        assert!(!exp.unregister("nonexistent"));
        assert_eq!(exp.variable_count(), 1);
        exp.clear_variables();
        assert_eq!(exp.variable_count(), 0);
    }

    #[test]
    fn template_expander_display() {
        let exp = UriTemplateExpander::new("{x}");
        let display = format!("{exp}");
        assert!(display.contains("0 vars"));
        assert!(display.contains("1 unresolved"));
    }

    // -- UriPathManipulator -------------------------------------------------

    #[test]
    fn path_manipulator_join() {
        let uri = VsUri::from_components("file", "", "/home/user", "", "");
        let joined = UriPathManipulator::join(&uri, "docs/readme.md");
        assert_eq!(joined.path, "/home/user/docs/readme.md");
    }

    #[test]
    fn path_manipulator_remove_dot_segments() {
        assert_eq!(
            UriPathManipulator::remove_dot_segments("/a/b/../c/./d"),
            "/a/c/d"
        );
        assert_eq!(UriPathManipulator::remove_dot_segments("/a/b/c"), "/a/b/c");
    }

    #[test]
    fn path_manipulator_depth() {
        let uri = VsUri::from_components("file", "", "/a/b/c", "", "");
        assert_eq!(UriPathManipulator::depth(&uri), 3);
        let root = VsUri::from_components("file", "", "/", "", "");
        assert_eq!(UriPathManipulator::depth(&root), 0);
    }

    #[test]
    fn path_manipulator_is_child_of() {
        let parent = VsUri::from_components("file", "", "/workspace", "", "");
        let child = VsUri::from_components("file", "", "/workspace/src/main.rs", "", "");
        let sibling = VsUri::from_components("file", "", "/other/path", "", "");
        assert!(UriPathManipulator::is_child_of(&parent, &child));
        assert!(!UriPathManipulator::is_child_of(&parent, &sibling));
    }

    #[test]
    fn path_manipulator_common_prefix() {
        let a = VsUri::from_components("file", "", "/workspace/src/lib.rs", "", "");
        let b = VsUri::from_components("file", "", "/workspace/src/main.rs", "", "");
        assert_eq!(UriPathManipulator::common_prefix(&a, &b), "/workspace/src");
    }

    // -- UriSchemeRegistry --------------------------------------------------

    #[test]
    fn scheme_registry_defaults() {
        let reg = UriSchemeRegistry::with_defaults();
        assert!(reg.is_known("http"));
        assert!(reg.is_known("https"));
        assert!(reg.is_known("ftp"));
        assert!(reg.is_known("ssh"));
        assert!(reg.is_known("file"));
        assert!(!reg.is_known("gopher"));
    }

    #[test]
    fn scheme_registry_default_ports() {
        let reg = UriSchemeRegistry::with_defaults();
        assert_eq!(reg.default_port("http"), Some(80));
        assert_eq!(reg.default_port("https"), Some(443));
        assert_eq!(reg.default_port("file"), None);
    }

    #[test]
    fn scheme_registry_is_secure() {
        let reg = UriSchemeRegistry::with_defaults();
        assert!(reg.is_secure("https"));
        assert!(reg.is_secure("ssh"));
        assert!(!reg.is_secure("http"));
        assert!(!reg.is_secure("ftp"));
    }

    #[test]
    fn scheme_registry_custom_registration() {
        let mut reg = UriSchemeRegistry::with_defaults();
        reg.register(UriSchemeInfo {
            name: "wss".into(),
            default_port: Some(443),
            is_secure: true,
            description: "WebSocket Secure".into(),
        });
        assert!(reg.is_known("wss"));
        assert!(reg.is_secure("wss"));
        assert_eq!(reg.default_port("wss"), Some(443));
    }

    #[test]
    fn scheme_registry_display_and_len() {
        let reg = UriSchemeRegistry::with_defaults();
        assert_eq!(reg.len(), 5);
        assert!(!reg.is_empty());
        let display = format!("{reg}");
        assert!(display.contains("5 schemes"));
    }


    // -- uri extended domain tests ----------------------------------------

    #[test]
    fn y_uri_enum_index() {
        assert_eq!(YUriUriScheme::File.index(), 0);
        assert_eq!(YUriUriScheme::Http.index(), 1);
        assert_eq!(YUriUriScheme::Https.index(), 2);
        assert_eq!(YUriUriScheme::Custom.index(), 3);
    }

    #[test]
    fn y_uri_enum_label() {
        assert_eq!(YUriUriScheme::File.label(), "File");
        assert_eq!(YUriUriScheme::Http.label(), "Http");
        assert_eq!(YUriUriScheme::Https.label(), "Https");
        assert_eq!(YUriUriScheme::Custom.label(), "Custom");
    }

    #[test]
    fn y_uri_enum_all() {
        let all = YUriUriScheme::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_uri_enum_is_default() {
        assert!(YUriUriScheme::File.is_default());
        assert!(!YUriUriScheme::Custom.is_default());
    }

    #[test]
    fn y_uri_enum_display() {
        assert_eq!(format!("{}", YUriUriScheme::File), "File");
    }

    #[test]
    fn y_uri_struct_new() {
        let s = YUriUriComponents::new();
        let _ = s.summary();
    }

    #[test]
    fn y_uri_fingerprint_deterministic() {
        let h1 = y_uri_fingerprint("hello");
        let h2 = y_uri_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_uri_fingerprint("a"), y_uri_fingerprint("b"));
    }

    #[test]
    fn y_uri_truncate_short() {
        assert_eq!(y_uri_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_uri_truncate_long() {
        let r = y_uri_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_uri_normalize_key_basic() {
        assert_eq!(y_uri_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_uri_split_path_basic() {
        let parts = y_uri_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_uri_count_occurrences_basic() {
        assert_eq!(y_uri_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_uri_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_uri_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_uri_in_range_basic() {
        assert!(y_uri_in_range(5, 1, 10));
        assert!(y_uri_in_range(1, 1, 10));
        assert!(y_uri_in_range(10, 1, 10));
        assert!(!y_uri_in_range(0, 1, 10));
        assert!(!y_uri_in_range(11, 1, 10));
    }

    #[test]
    fn y_uri_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_uri_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_uri_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_uri_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- uri Z-extended tests -----------------------------------------------

    #[test]
    fn z_uri_priority_weight() {
        assert_eq!(ZUriPriority::Idle.weight(), 0);
        assert_eq!(ZUriPriority::Normal.weight(), 2);
        assert_eq!(ZUriPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_uri_priority_label() {
        assert_eq!(ZUriPriority::Low.label(), "low");
        assert_eq!(ZUriPriority::High.label(), "high");
    }

    #[test]
    fn z_uri_priority_is_elevated() {
        assert!(!ZUriPriority::Normal.is_elevated());
        assert!(ZUriPriority::High.is_elevated());
        assert!(ZUriPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_uri_priority_display() {
        assert_eq!(format!("{}", ZUriPriority::Idle), "idle");
    }

    #[test]
    fn z_uri_priority_all_asc() {
        let all = ZUriPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZUriPriority::Idle);
        assert_eq!(all[4], ZUriPriority::Realtime);
    }

    #[test]
    fn z_uri_struct_new() {
        let s = ZUriUriTemplateExpander::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_uri_struct_toggled_clone() {
        let s = ZUriUriTemplateExpander::new();
        let t = s.toggled_clone();
        assert_ne!(s.strict, t.strict);
    }

    #[test]
    fn z_uri_rolling_hash_deterministic() {
        let h1 = z_uri_rolling_hash(b"test");
        let h2 = z_uri_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_uri_rolling_hash(b"a"), z_uri_rolling_hash(b"b"));
    }

    #[test]
    fn z_uri_pad_to_basic() {
        assert_eq!(z_uri_pad_to("hi", 5), "hi   ");
        assert_eq!(z_uri_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_uri_is_identifier_basic() {
        assert!(z_uri_is_identifier("foo_bar"));
        assert!(z_uri_is_identifier("abc123"));
        assert!(!z_uri_is_identifier(""));
        assert!(!z_uri_is_identifier("has space"));
    }

    #[test]
    fn z_uri_levenshtein_basic() {
        assert_eq!(z_uri_levenshtein("", ""), 0);
        assert_eq!(z_uri_levenshtein("abc", "abc"), 0);
        assert_eq!(z_uri_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_uri_unique_words_basic() {
        let w = z_uri_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_uri_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_uri_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_uri_common_prefix_basic() {
        assert_eq!(z_uri_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_uri_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_uri_struct_clear() {
        let mut s = ZUriUriTemplateExpander::new();
        s.variables.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_uri_rolling_hash_empty() {
        let h = z_uri_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_93_push_and_len() {
        let mut rb = super::XbRingBuffer93::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_93_overwrite() {
        let mut rb = super::XbRingBuffer93::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_93_get_out_of_bounds() {
        let rb = super::XbRingBuffer93::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_93_drain_all() {
        let mut rb = super::XbRingBuffer93::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_93_peek_front_back() {
        let mut rb = super::XbRingBuffer93::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_93_clear() {
        let mut rb = super::XbRingBuffer93::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_93_capacity() {
        let rb = super::XbRingBuffer93::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_93_basic() {
        let h = super::xb_fnv1a_93(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_93(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_93_different_inputs() {
        let h1 = super::xb_fnv1a_93(b"abc");
        let h2 = super::xb_fnv1a_93(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_93_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_93(&data);
        let dec = super::xb_rle_decode_93(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_93_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_93(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_93(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_93_values() {
        assert!((super::xb_clamp_93(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_93(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_93(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_93_values() {
        assert!((super::xb_lerp_93(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_93(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_93(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_93_wrap_around_twice() {
        let mut rb = super::XbRingBuffer93::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 192 ----

    #[test]
    fn xc_192_pool_new_empty() {
        let pool: super::Xc192Pool<i32> = super::Xc192Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_192_pool_release_acquire() {
        let mut pool = super::Xc192Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_192_pool_acquire_empty() {
        let mut pool: super::Xc192Pool<i32> = super::Xc192Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_192_pool_full() {
        let mut pool = super::Xc192Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_192_pool_drain() {
        let mut pool = super::Xc192Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_192_pool_stats() {
        let mut pool = super::Xc192Pool::new(8);
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
    fn xc_192_pool_clear() {
        let mut pool = super::Xc192Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_192_pool_shrink() {
        let mut pool = super::Xc192Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_192_pool_default() {
        let pool: super::Xc192Pool<String> = super::Xc192Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_192_pool_extend() {
        let mut pool = super::Xc192Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_192_pool_retain() {
        let mut pool = super::Xc192Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_192_scheduler_round_robin() {
        let mut sched = super::Xc192Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_192_scheduler_empty() {
        let mut sched = super::Xc192Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_192_scheduler_reset() {
        let mut sched = super::Xc192Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_192_scheduler_add_remove() {
        let mut sched = super::Xc192Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_192_scheduler_targets() {
        let sched = super::Xc192Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_192_hash_empty() {
        assert_eq!(super::xc_192_hash(b""), 5381);
    }

    #[test]
    fn xc_192_hash_data() {
        let h = super::xc_192_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_192_hash(b"hello"), h);
    }

    #[test]
    fn xc_192_reverse_str() {
        assert_eq!(super::xc_192_reverse("abc"), "cba");
        assert_eq!(super::xc_192_reverse(""), "");
    }


    #[test]
    fn xe_106_pipeline_empty() {
        let p = super::Xe106Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_106_pipeline_parse_stage() {
        let p = super::Xe106Pipeline::new()
            .add_parse(super::xe_106_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_106_pipeline_transform_double() {
        let p = super::Xe106Pipeline::new()
            .add_transform(super::xe_106_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_106_pipeline_validate_reverse() {
        let p = super::Xe106Pipeline::new()
            .add_validate(super::xe_106_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_106_pipeline_emit_filter() {
        let p = super::Xe106Pipeline::new()
            .add_emit(super::xe_106_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_106_pipeline_multi_stage() {
        let p = super::Xe106Pipeline::new()
            .add_parse(super::xe_106_pipeline_identity)
            .add_transform(super::xe_106_pipeline_double)
            .add_validate(super::xe_106_pipeline_reverse)
            .add_emit(super::xe_106_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_106_pipeline_error_propagation() {
        let p = super::Xe106Pipeline::new()
            .add_parse(super::xe_106_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe106Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_106_pipeline_compose() {
        let p1 = super::Xe106Pipeline::new()
            .add_parse(super::xe_106_pipeline_identity);
        let p2 = super::Xe106Pipeline::new()
            .add_transform(super::xe_106_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_106_pipeline_error_display() {
        let e = super::Xe106PipelineError {
            stage: super::Xe106Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_106_cache_put_get() {
        let mut c = super::Xe106Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_106_cache_miss() {
        let mut c: super::Xe106Cache<&str, i32> = super::Xe106Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_106_cache_ttl_expiry() {
        let mut c = super::Xe106Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_106_cache_evict() {
        let mut c = super::Xe106Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_106_cache_capacity() {
        let mut c = super::Xe106Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_106_cache_stats() {
        let mut c = super::Xe106Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_106_cache_clear() {
        let mut c = super::Xe106Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_104 graph tests ------------------------------------------------

    #[test]
    fn xg_104_graph_empty() {
        let g = super::Xg104Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_104_graph_add_node() {
        let mut g = super::Xg104Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_104_graph_add_edge() {
        let mut g = super::Xg104Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_104_graph_neighbors() {
        let mut g = super::Xg104Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_104_graph_has_path() {
        let mut g = super::Xg104Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_104_graph_self_path() {
        let g = super::Xg104Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_104_graph_topo_sort() {
        let mut g = super::Xg104Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_104_graph_cycle_detect_false() {
        let mut g = super::Xg104Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_104_graph_cycle_detect_true() {
        let mut g = super::Xg104Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_104 heap tests -------------------------------------------------

    #[test]
    fn xg_104_heap_empty() {
        let h: super::Xg104Heap<i32> = super::Xg104Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_104_heap_push_pop() {
        let mut h = super::Xg104Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_104_heap_peek() {
        let mut h = super::Xg104Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_104_heap_drain_sorted() {
        let mut h = super::Xg104Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_104_heap_merge() {
        let mut a = super::Xg104Heap::new();
        let mut b = super::Xg104Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_104_heap_default() {
        let h: super::Xg104Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_104_graph_default() {
        let g: super::Xg104Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh191_skip_insert_contains() {
        let mut sl = super::Xh191SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh191_skip_remove() {
        let mut sl = super::Xh191SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh191_skip_len() {
        let mut sl = super::Xh191SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh191_skip_range_query() {
        let mut sl = super::Xh191SkipList::xh_new(4);
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
    fn xh191_skip_floor_ceiling() {
        let mut sl = super::Xh191SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh191_skip_rank() {
        let mut sl = super::Xh191SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh191_skip_empty() {
        let sl = super::Xh191SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh191_skip_duplicates() {
        let mut sl = super::Xh191SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh191_bitset_set_test() {
        let mut bs = super::Xh191BitSet::xh_new(256);
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
    fn xh191_bitset_clear_count() {
        let mut bs = super::Xh191BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh191_bitset_and_or_xor() {
        let mut a = super::Xh191BitSet::xh_new(128);
        let mut b = super::Xh191BitSet::xh_new(128);
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
    fn xh191_bitset_iter_ones() {
        let mut bs = super::Xh191BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh191_bitset_first_last() {
        let mut bs = super::Xh191BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh191_bitset_empty() {
        let bs = super::Xh191BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi191_deque_push_pop_back() {
        let mut dq = super::Xi191Deque::xi_new(4);
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
    fn xi191_deque_push_pop_front() {
        let mut dq = super::Xi191Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi191_deque_mixed_ops() {
        let mut dq = super::Xi191Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi191_deque_get_and_split() {
        let mut dq = super::Xi191Deque::xi_new(8);
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
    fn xi191_deque_rotate_left() {
        let mut dq = super::Xi191Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi191_deque_rotate_right() {
        let mut dq = super::Xi191Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi191_deque_grow() {
        let mut dq = super::Xi191Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi191_deque_empty() {
        let dq = super::Xi191Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi191_interval_tree_insert_query() {
        let mut tree = super::Xi191IntervalTree::xi_new();
        tree.xi_insert(super::Xi191Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi191Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi191Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi191_interval_tree_overlap() {
        let mut tree = super::Xi191IntervalTree::xi_new();
        tree.xi_insert(super::Xi191Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi191Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi191Interval::xi_new(12, 20));
        let q = super::Xi191Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi191_interval_tree_remove() {
        let mut tree = super::Xi191IntervalTree::xi_new();
        tree.xi_insert(super::Xi191Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi191Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi191_interval_tree_gaps() {
        let mut tree = super::Xi191IntervalTree::xi_new();
        tree.xi_insert(super::Xi191Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi191Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi191Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi191Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi191Interval::xi_new(8, 10));
    }

    #[test]
    fn xi191_interval_tree_merge() {
        let mut tree = super::Xi191IntervalTree::xi_new();
        tree.xi_insert(super::Xi191Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi191Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi191Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi191Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi191Interval::xi_new(10, 15));
    }

    #[test]
    fn xi191_interval_tree_all() {
        let mut tree = super::Xi191IntervalTree::xi_new();
        tree.xi_insert(super::Xi191Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi191Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi191_interval_tree_empty() {
        let tree = super::Xi191IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi191_interval_tree_contains_point() {
        let iv = super::Xi191Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 192) ---

    #[test]
    fn xj_192_uf_make_and_find() {
        let mut uf = super::Xj192UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_192_uf_union_connected() {
        let mut uf = super::Xj192UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_192_uf_component_count() {
        let mut uf = super::Xj192UnionFind::xj_new();
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
    fn xj_192_uf_component_size() {
        let mut uf = super::Xj192UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_192_uf_largest_component() {
        let mut uf = super::Xj192UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_192_uf_many_elements() {
        let mut uf = super::Xj192UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_192_uf_separate_components() {
        let mut uf = super::Xj192UnionFind::xj_new();
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
    fn xj_192_uf_path_compression() {
        let mut uf = super::Xj192UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_192_bt_insert_get() {
        let mut bt = super::Xj192BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_192_bt_contains_len() {
        let mut bt = super::Xj192BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_192_bt_replace() {
        let mut bt = super::Xj192BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_192_bt_remove() {
        let mut bt = super::Xj192BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_192_bt_keys_values() {
        let mut bt = super::Xj192BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_192_bt_range() {
        let mut bt = super::Xj192BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_192_bt_min_max() {
        let mut bt = super::Xj192BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_192_bt_many_inserts() {
        let mut bt = super::Xj192BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_191 segment tree tests ---

    #[test]
    fn xk_191_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk191SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_191_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk191SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_191_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk191SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_191_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk191SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_191_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk191SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_191_st_single_element() {
        let data = vec![42];
        let st = super::Xk191SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_191_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk191SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_191_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk191SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_191 disjoint intervals tests ---

    #[test]
    fn xk_191_di_add_and_count() {
        let mut di = super::Xk191DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_191_di_merge_overlap() {
        let mut di = super::Xk191DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_191_di_contains() {
        let mut di = super::Xk191DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_191_di_remove() {
        let mut di = super::Xk191DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_191_di_covered_length() {
        let mut di = super::Xk191DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_191_di_gaps() {
        let mut di = super::Xk191DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_191_di_merge_adjacent() {
        let mut di = super::Xk191DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_191_di_empty() {
        let di = super::Xk191DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_192_rope_new_empty() {
        let rope = super::Xl192Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_192_rope_from_str() {
        let rope = super::Xl192Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_192_rope_insert_at() {
        let mut rope = super::Xl192Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_192_rope_delete_range() {
        let mut rope = super::Xl192Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_192_rope_char_at() {
        let rope = super::Xl192Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_192_rope_split_concat() {
        let rope = super::Xl192Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_192_rope_line_count() {
        let rope = super::Xl192Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_192_rope_line_at() {
        let rope = super::Xl192Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_192_sa_build_and_search() {
        let sa = super::Xl192SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_192_sa_count() {
        let sa = super::Xl192SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_192_sa_longest_repeated() {
        let sa = super::Xl192SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_192_sa_all_positions() {
        let sa = super::Xl192SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_192_sa_len() {
        let sa = super::Xl192SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_192_sa_empty() {
        let sa = super::Xl192SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_192_rope_slice() {
        let rope = super::Xl192Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_192_sa_search_start() {
        let sa = super::Xl192SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
