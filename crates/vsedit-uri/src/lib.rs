//! URI parsing and resource identification.
//!
//! Equivalent to VS Code's `vs/base/common/uri.ts`. The [`VsUri`] type is the
//! fundamental resource identifier used throughout the editor.

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
// Tests
// ---------------------------------------------------------------------------

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
}
