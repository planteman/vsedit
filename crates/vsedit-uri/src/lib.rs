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
        // Non-ASCII bytes should be percent-encoded.
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
        // UNC paths don't have drive letters, leading `/` is stripped.
        assert_eq!(result, "server\\share\\file.txt");
    }

    // -- serde roundtrip ----------------------------------------------------

    #[test]
    fn serde_serialize() {
        let uri = VsUri::file("/home/user/file.rs");
        let json = serde_json::to_string(&uri).unwrap();
        // Should be a JSON string containing the URI.
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
        // The `#` in the path must be encoded in the URI string.
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
}
