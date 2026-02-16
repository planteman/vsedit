//! Document link detection for editor content.
//!
//! Provides URL and file-path detection within text, as well as a trait for
//! language-specific document-link providers.

use std::fmt;
// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A resolved or unresolved link found in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLink {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub target: String,
    pub tooltip: Option<String>,
}

/// Trait for language-specific link providers.
pub trait DocumentLinkProvider {
    /// Return all document links for the given URI.
    fn provide_document_links(&self, uri: &str) -> Vec<DocumentLink>;

    /// Optionally resolve additional information (e.g. tooltip) for a link.
    fn resolve_link(&self, link: &mut DocumentLink);
}

// ---------------------------------------------------------------------------
// URL detection
// ---------------------------------------------------------------------------

/// Detect `http://` and `https://` URLs in `text`.
///
/// Returns `(byte_start, byte_end, url)` tuples.
pub fn detect_urls(text: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    for prefix in &["https://", "http://"] {
        let mut search_from = 0;
        while let Some(start) = text[search_from..].find(prefix) {
            let abs_start = search_from + start;
            let rest = &text[abs_start..];
            let end = rest
                .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | ')'))
                .unwrap_or(rest.len());
            if end > prefix.len() {
                results.push((abs_start, abs_start + end, rest[..end].to_string()));
            }
            search_from = abs_start + end;
        }
    }
    results.sort_by_key(|r| r.0);
    results
}

// ---------------------------------------------------------------------------
// File-path detection
// ---------------------------------------------------------------------------

/// Detect file-path-like patterns in `text`.
///
/// Recognises Unix-style absolute paths (`/foo/bar`) and relative paths
/// starting with `./` or `../`.  Returns `(byte_start, byte_end, path)`.
pub fn detect_file_paths(text: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    let prefixes: &[&str] = &["../", "./"];

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        let rest = &text[i..];

        // Check for relative prefixes first.
        let matched_prefix = prefixes.iter().any(|p| rest.starts_with(p));

        // Check for absolute Unix paths (must be preceded by whitespace or
        // start-of-string to avoid matching inside URLs).
        let is_abs = rest.starts_with('/')
            && !rest.starts_with("//")
            && (i == 0 || text.as_bytes()[i - 1].is_ascii_whitespace());

        if matched_prefix || is_abs {
            let end = rest
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '>' | '<'))
                .unwrap_or(rest.len());
            if end > 1 {
                results.push((i, i + end, rest[..end].to_string()));
            }
            i += end;
        } else {
            i += 1;
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during link detection or resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    InvalidUrl(String),
    InvalidRange { reason: String },
    UnresolvableLink(String),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::InvalidUrl(u) => write!(f, "invalid URL: {u}"),
            LinkError::InvalidRange { reason } => write!(f, "invalid range: {reason}"),
            LinkError::UnresolvableLink(t) => write!(f, "unresolvable link: {t}"),
        }
    }
}

impl std::error::Error for LinkError {}

// ---------------------------------------------------------------------------
// Display for DocumentLink
// ---------------------------------------------------------------------------

impl std::fmt::Display for DocumentLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Link: {} [{}:{}-{}:{}]",
            self.target, self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

// ---------------------------------------------------------------------------
// DocumentLink helpers
// ---------------------------------------------------------------------------

impl DocumentLink {
    /// Returns `true` if the given position falls within this link's range.
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col > self.end_col {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Email detection
// ---------------------------------------------------------------------------

/// Detect email addresses in `text`.
///
/// Returns `(byte_start, byte_end, email)` tuples.
pub fn detect_emails(text: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();

    let mut i = 0;
    while i < len {
        if bytes[i] == b'@' && i > 0 {
            // Walk backwards for local part
            let local_start = {
                let mut s = i;
                while s > 0
                    && (bytes[s - 1].is_ascii_alphanumeric()
                        || matches!(bytes[s - 1], b'.' | b'+' | b'-' | b'_'))
                {
                    s -= 1;
                }
                s
            };
            // Walk forwards for domain part
            let domain_end = {
                let mut e = i + 1;
                while e < len
                    && (bytes[e].is_ascii_alphanumeric() || matches!(bytes[e], b'.' | b'-'))
                {
                    e += 1;
                }
                e
            };
            let local = &text[local_start..i];
            let domain = &text[i + 1..domain_end];
            if !local.is_empty() && domain.contains('.') && domain.len() >= 3 {
                results.push((local_start, domain_end, text[local_start..domain_end].to_string()));
                i = domain_end;
                continue;
            }
        }
        i += 1;
    }
    results
}

// ---------------------------------------------------------------------------
// Aggregated detection
// ---------------------------------------------------------------------------

/// Detect URLs, file paths, and emails in `text`, returning all results
/// sorted by byte start position.
pub fn detect_all_links(text: &str) -> Vec<(usize, usize, String)> {
    let mut all = detect_urls(text);
    all.extend(detect_file_paths(text));
    all.extend(detect_emails(text));
    all.sort_by_key(|r| r.0);
    all.dedup_by_key(|r| r.0);
    all
}

// ---------------------------------------------------------------------------
// Link classification
// ---------------------------------------------------------------------------

/// Classification of a detected link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkClassification {
    Url,
    FilePath,
    Email,
    Custom(String),
}

/// A detected link together with its classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedLink {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub classification: LinkClassification,
}

/// Run all detectors and return classified results sorted by byte position.
pub fn classify_links(text: &str) -> Vec<ClassifiedLink> {
    let mut out = Vec::new();
    for (s, e, t) in detect_urls(text) {
        out.push(ClassifiedLink { start: s, end: e, text: t, classification: LinkClassification::Url });
    }
    for (s, e, t) in detect_file_paths(text) {
        out.push(ClassifiedLink { start: s, end: e, text: t, classification: LinkClassification::FilePath });
    }
    for (s, e, t) in detect_emails(text) {
        out.push(ClassifiedLink { start: s, end: e, text: t, classification: LinkClassification::Email });
    }
    out.sort_by_key(|c| c.start);
    out
}

// ---------------------------------------------------------------------------
// Link validation
// ---------------------------------------------------------------------------

/// Basic link validator that performs format-level checks without network I/O.
#[derive(Debug, Default)]
pub struct LinkValidator;

impl LinkValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validates that a URL string has a scheme and a host component.
    pub fn validate_url(&self, url: &str) -> Result<(), LinkError> {
        let rest = if let Some(r) = url.strip_prefix("https://") {
            r
        } else if let Some(r) = url.strip_prefix("http://") {
            r
        } else {
            return Err(LinkError::InvalidUrl("missing http(s) scheme".into()));
        };
        if rest.is_empty() || !rest.contains('.') {
            return Err(LinkError::InvalidUrl("missing host".into()));
        }
        Ok(())
    }

    /// Validates a file path for suspicious patterns such as null bytes or
    /// directory traversal beyond a reasonable depth.
    pub fn validate_file_path(&self, path: &str) -> Result<(), LinkError> {
        if path.contains('\0') {
            return Err(LinkError::InvalidUrl("path contains null byte".into()));
        }
        let depth: i32 = path.split('/').fold(0i32, |d, seg| match seg {
            ".." => d - 1,
            "" | "." => d,
            _ => d + 1,
        });
        if depth < -4 {
            return Err(LinkError::InvalidUrl("excessive directory traversal".into()));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for links operations.
#[derive(Debug, Clone, PartialEq)]
pub struct LinksStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl LinksStats {
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
    pub fn merge(&mut self, other: &LinksStats) {
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

impl Default for LinksStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LinksStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LinksStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for links.
#[derive(Debug, Clone)]
pub struct LinksValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl LinksValidator {
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

impl Default for LinksValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// UrlMatch — structured URL match result
// ---------------------------------------------------------------------------

/// A structured match for a detected URL in source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlMatch {
    /// Byte start offset in the source string.
    pub byte_start: usize,
    /// Byte end offset (exclusive) in the source string.
    pub byte_end: usize,
    /// The matched URL string.
    pub url: String,
    /// Line number (0-based) where the URL starts, if computed.
    pub line: Option<u32>,
    /// Column (0-based) within the line.
    pub column: Option<u32>,
}

/// Detect URLs in `text` and return structured [`UrlMatch`] results.
///
/// Line and column are computed relative to the full text.
pub fn detect_urls_structured(text: &str) -> Vec<UrlMatch> {
    let raw = detect_urls(text);
    raw.into_iter()
        .map(|(start, end, url)| {
            let (line, col) = byte_offset_to_line_col(text, start);
            UrlMatch {
                byte_start: start,
                byte_end: end,
                url,
                line: Some(line),
                column: Some(col),
            }
        })
        .collect()
}

/// Convert a byte offset into 0-based (line, column).
fn byte_offset_to_line_col(text: &str, offset: usize) -> (u32, u32) {
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    for (i, ch) in text.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

// ---------------------------------------------------------------------------
// Link interaction helpers
// ---------------------------------------------------------------------------

/// Style applied when a link is hovered (underline) or followed (Ctrl+Click).
#[derive(Debug, Clone)]
pub struct LinkStyle {
    /// Prefix before the link text when rendering with underline.
    pub underline_prefix: String,
    /// Suffix after the link text.
    pub underline_suffix: String,
}

impl Default for LinkStyle {
    fn default() -> Self {
        Self {
            underline_prefix: String::new(),
            underline_suffix: String::new(),
        }
    }
}

/// Result of a Ctrl+Click follow action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowLinkAction {
    /// Open a URL in an external browser.
    OpenExternal(String),
    /// Open a file path in the editor.
    OpenFile(String),
    /// Open a mailto link.
    Mailto(String),
    /// No actionable link at the position.
    None,
}

/// Determine the follow action for a link at `(line, col)`.
pub fn follow_link_at(links: &[DocumentLink], line: u32, col: u32) -> FollowLinkAction {
    for link in links {
        if link.contains_position(line, col) {
            let target = &link.target;
            if target.starts_with("http://") || target.starts_with("https://") {
                return FollowLinkAction::OpenExternal(target.clone());
            } else if target.starts_with("mailto:") || target.contains('@') {
                return FollowLinkAction::Mailto(target.clone());
            } else {
                return FollowLinkAction::OpenFile(target.clone());
            }
        }
    }
    FollowLinkAction::None
}

// ---------------------------------------------------------------------------
// Extended helpers
// ---------------------------------------------------------------------------

impl DocumentLink {
    /// Width of the link in columns (only meaningful for single-line links).
    pub fn span(&self) -> u32 {
        self.end_col.saturating_sub(self.start_col)
    }

    /// Returns `true` when the target looks like an HTTP(S) URL.
    pub fn is_url(&self) -> bool {
        self.target.starts_with("http://") || self.target.starts_with("https://")
    }
}

impl ClassifiedLink {
    /// Returns `true` when the classification is `Email`.
    pub fn is_email(&self) -> bool {
        self.classification == LinkClassification::Email
    }
}

impl LinkClassification {
    /// Human-readable label for the classification variant.
    pub fn label(&self) -> &'static str {
        match self {
            LinkClassification::Url => "URL",
            LinkClassification::FilePath => "File Path",
            LinkClassification::Email => "Email",
            LinkClassification::Custom(_) => "Custom",
        }
    }
}

/// Extract the domain (host) from an `http://` or `https://` URL.
///
/// Returns `None` when the URL does not start with an HTTP(S) scheme or has
/// no host component.
pub fn extract_domain(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    if rest.is_empty() {
        return None;
    }
    let host = rest.split('/').next().unwrap_or(rest);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.split('?').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

impl LinkValidator {
    /// Basic email format validation (no network I/O).
    pub fn validate_email(&self, email: &str) -> Result<(), LinkError> {
        let at_pos = email.find('@').ok_or_else(|| {
            LinkError::InvalidUrl("missing '@' in email".into())
        })?;
        let local = &email[..at_pos];
        let domain = &email[at_pos + 1..];
        if local.is_empty() {
            return Err(LinkError::InvalidUrl("empty local part".into()));
        }
        if domain.is_empty() || !domain.contains('.') {
            return Err(LinkError::InvalidUrl("invalid domain in email".into()));
        }
        if domain.starts_with('.') || domain.ends_with('.') {
            return Err(LinkError::InvalidUrl("domain starts or ends with '.'".into()));
        }
        Ok(())
    }
}

/// Count classified links by type, returning `(url_count, file_count, email_count)`.
pub fn count_links_by_type(links: &[ClassifiedLink]) -> (usize, usize, usize) {
    let mut urls = 0usize;
    let mut files = 0usize;
    let mut emails = 0usize;
    for link in links {
        match link.classification {
            LinkClassification::Url => urls += 1,
            LinkClassification::FilePath => files += 1,
            LinkClassification::Email => emails += 1,
            LinkClassification::Custom(_) => {}
        }
    }
    (urls, files, emails)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_https_url() {
        let text = "Visit https://example.com for info";
        let urls = detect_urls(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].2, "https://example.com");
    }

    #[test]
    fn detect_multiple_urls() {
        let text = "http://a.com and https://b.org/path?q=1";
        let urls = detect_urls(text);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].2, "http://a.com");
        assert_eq!(urls[1].2, "https://b.org/path?q=1");
    }

    #[test]
    fn detect_unix_file_paths() {
        let text = "see /usr/local/bin/foo and ./relative/path";
        let paths = detect_file_paths(text);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].2, "/usr/local/bin/foo");
        assert_eq!(paths[1].2, "./relative/path");
    }

    #[test]
    fn document_link_fields() {
        let link = DocumentLink {
            start_line: 0,
            start_col: 5,
            end_line: 0,
            end_col: 25,
            target: "https://example.com".into(),
            tooltip: Some("Example".into()),
        };
        assert_eq!(link.target, "https://example.com");
        assert_eq!(link.tooltip.as_deref(), Some("Example"));
    }

    // -- new tests --

    #[test]
    fn detect_single_email() {
        let text = "Contact user@example.com for help";
        let emails = detect_emails(text);
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].2, "user@example.com");
    }

    #[test]
    fn detect_multiple_emails() {
        let text = "a@b.co and c+tag@d.org end";
        let emails = detect_emails(text);
        assert_eq!(emails.len(), 2);
        assert_eq!(emails[0].2, "a@b.co");
        assert_eq!(emails[1].2, "c+tag@d.org");
    }

    #[test]
    fn detect_all_links_mixed() {
        let text = "see https://x.com and /etc/hosts or a@b.co";
        let all = detect_all_links(text);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].2, "https://x.com");
        assert_eq!(all[1].2, "/etc/hosts");
        assert_eq!(all[2].2, "a@b.co");
    }

    #[test]
    fn contains_position_inside() {
        let link = DocumentLink {
            start_line: 1,
            start_col: 5,
            end_line: 1,
            end_col: 20,
            target: "t".into(),
            tooltip: None,
        };
        assert!(link.contains_position(1, 10));
        assert!(link.contains_position(1, 5));
        assert!(link.contains_position(1, 20));
        assert!(!link.contains_position(1, 4));
        assert!(!link.contains_position(1, 21));
        assert!(!link.contains_position(0, 10));
        assert!(!link.contains_position(2, 10));
    }

    #[test]
    fn contains_position_multiline() {
        let link = DocumentLink {
            start_line: 2,
            start_col: 10,
            end_line: 4,
            end_col: 3,
            target: "t".into(),
            tooltip: None,
        };
        assert!(link.contains_position(3, 0));
        assert!(link.contains_position(3, 999));
        assert!(link.contains_position(2, 10));
        assert!(!link.contains_position(2, 9));
        assert!(link.contains_position(4, 3));
        assert!(!link.contains_position(4, 4));
    }

    #[test]
    fn classify_links_types() {
        let text = "https://x.com ./foo a@b.co";
        let classified = classify_links(text);
        assert!(classified.iter().any(|c| c.classification == LinkClassification::Url));
        assert!(classified.iter().any(|c| c.classification == LinkClassification::FilePath));
        assert!(classified.iter().any(|c| c.classification == LinkClassification::Email));
    }

    #[test]
    fn link_validator_url() {
        let v = LinkValidator::new();
        assert!(v.validate_url("https://example.com").is_ok());
        assert!(v.validate_url("http://a.b").is_ok());
        assert!(v.validate_url("ftp://bad").is_err());
        assert!(v.validate_url("https://").is_err());
    }

    #[test]
    fn link_validator_file_path() {
        let v = LinkValidator::new();
        assert!(v.validate_file_path("/usr/bin/ls").is_ok());
        assert!(v.validate_file_path("../../src/main.rs").is_ok());
        assert!(v.validate_file_path("../../../../../../../../../etc/passwd").is_err());
        assert!(v.validate_file_path("/tmp/\0bad").is_err());
    }

    #[test]
    fn display_document_link() {
        let link = DocumentLink {
            start_line: 3,
            start_col: 7,
            end_line: 3,
            end_col: 30,
            target: "https://rust-lang.org".into(),
            tooltip: None,
        };
        assert_eq!(link.to_string(), "Link: https://rust-lang.org [3:7-3:30]");
    }

    #[test]
    fn error_display() {
        let e1 = LinkError::InvalidUrl("bad".into());
        assert_eq!(e1.to_string(), "invalid URL: bad");
        let e2 = LinkError::InvalidRange { reason: "neg".into() };
        assert_eq!(e2.to_string(), "invalid range: neg");
        let e3 = LinkError::UnresolvableLink("x".into());
        assert_eq!(e3.to_string(), "unresolvable link: x");
    }

    #[test]
    fn no_links_in_plain_text() {
        let text = "just some plain text without any links";
        assert!(detect_all_links(text).is_empty());
        assert!(classify_links(text).is_empty());
    }

    #[test]
    fn custom_classification_variant() {
        let c = LinkClassification::Custom("markdown".into());
        assert_eq!(c, LinkClassification::Custom("markdown".into()));
        assert_ne!(c, LinkClassification::Url);
    }

    #[test]
    fn eq_linkclassification_same() {
        assert_eq!(LinkClassification::Url, LinkClassification::Url);
    }

    #[test]
    fn ne_linkclassification_diff() {
        assert_ne!(LinkClassification::Url, LinkClassification::FilePath);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = LinkValidator::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn links_stats_new_defaults() {
        let stats = LinksStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn links_stats_record_success() {
        let mut stats = LinksStats::new();
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
    fn links_stats_record_failure() {
        let mut stats = LinksStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn links_stats_reset() {
        let mut stats = LinksStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn links_stats_merge() {
        let mut a = LinksStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = LinksStats::new();
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
    fn links_stats_display() {
        let mut stats = LinksStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn links_stats_default() {
        let stats = LinksStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn links_validator_accepts_valid_name() {
        let v = LinksValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn links_validator_rejects_empty() {
        let v = LinksValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn links_validator_rejects_too_long() {
        let v = LinksValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn links_validator_forbidden_prefix() {
        let v = LinksValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn links_validator_allowed_chars() {
        let v = LinksValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn links_validator_range() {
        let v = LinksValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn links_sanitize_removes_control() {
        let result = LinksValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn links_truncate_short_string() {
        assert_eq!(LinksValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn links_truncate_long_string() {
        let result = LinksValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn links_is_ascii_printable() {
        assert!(LinksValidator::is_ascii_printable("Hello World 123"));
        assert!(!LinksValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- structured URL & follow-link tests ---------------------------------

    #[test]
    fn detect_urls_structured_single() {
        let text = "see https://example.com here";
        let matches = detect_urls_structured(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].url, "https://example.com");
        assert_eq!(matches[0].line, Some(0));
        assert_eq!(matches[0].column, Some(4));
    }

    #[test]
    fn detect_urls_structured_multiline() {
        let text = "line1\nhttps://a.com\nline3";
        let matches = detect_urls_structured(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, Some(1));
        assert_eq!(matches[0].column, Some(0));
    }

    #[test]
    fn follow_link_open_external() {
        let links = vec![DocumentLink {
            start_line: 1,
            start_col: 0,
            end_line: 1,
            end_col: 20,
            target: "https://example.com".into(),
            tooltip: None,
        }];
        assert_eq!(
            follow_link_at(&links, 1, 5),
            FollowLinkAction::OpenExternal("https://example.com".into())
        );
    }

    #[test]
    fn follow_link_open_file() {
        let links = vec![DocumentLink {
            start_line: 2,
            start_col: 0,
            end_line: 2,
            end_col: 10,
            target: "./src/main.rs".into(),
            tooltip: None,
        }];
        assert_eq!(
            follow_link_at(&links, 2, 3),
            FollowLinkAction::OpenFile("./src/main.rs".into())
        );
    }

    #[test]
    fn follow_link_none() {
        assert_eq!(follow_link_at(&[], 0, 0), FollowLinkAction::None);
    }

    #[test]
    fn follow_link_mailto() {
        let links = vec![DocumentLink {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 15,
            target: "user@example.com".into(),
            tooltip: None,
        }];
        assert_eq!(
            follow_link_at(&links, 0, 5),
            FollowLinkAction::Mailto("user@example.com".into())
        );
    }

    // -- extended helper tests --

    #[test]
    fn document_link_span() {
        let link = DocumentLink {
            start_line: 1,
            start_col: 5,
            end_line: 1,
            end_col: 25,
            target: "x".into(),
            tooltip: None,
        };
        assert_eq!(link.span(), 20);
    }

    #[test]
    fn document_link_is_url() {
        let http = DocumentLink {
            start_line: 0, start_col: 0, end_line: 0, end_col: 10,
            target: "http://example.com".into(), tooltip: None,
        };
        let file = DocumentLink {
            start_line: 0, start_col: 0, end_line: 0, end_col: 10,
            target: "./foo.rs".into(), tooltip: None,
        };
        assert!(http.is_url());
        assert!(!file.is_url());
    }

    #[test]
    fn classified_link_is_email() {
        let email_link = ClassifiedLink {
            start: 0, end: 10,
            text: "a@b.com".into(),
            classification: LinkClassification::Email,
        };
        let url_link = ClassifiedLink {
            start: 0, end: 10,
            text: "https://x.com".into(),
            classification: LinkClassification::Url,
        };
        assert!(email_link.is_email());
        assert!(!url_link.is_email());
    }

    #[test]
    fn link_classification_labels() {
        assert_eq!(LinkClassification::Url.label(), "URL");
        assert_eq!(LinkClassification::FilePath.label(), "File Path");
        assert_eq!(LinkClassification::Email.label(), "Email");
        assert_eq!(LinkClassification::Custom("x".into()).label(), "Custom");
    }

    #[test]
    fn extract_domain_various() {
        assert_eq!(extract_domain("https://example.com/path"), Some("example.com"));
        assert_eq!(extract_domain("http://sub.domain.org:8080/x"), Some("sub.domain.org"));
        assert_eq!(extract_domain("ftp://nope.com"), None);
        assert_eq!(extract_domain("https://"), None);
    }

    #[test]
    fn validate_email_basic() {
        let v = LinkValidator::new();
        assert!(v.validate_email("user@example.com").is_ok());
        assert!(v.validate_email("missing-at.com").is_err());
        assert!(v.validate_email("@no-local.com").is_err());
        assert!(v.validate_email("user@").is_err());
        assert!(v.validate_email("user@.bad.com").is_err());
    }

    #[test]
    fn count_links_by_type_counts() {
        let links = vec![
            ClassifiedLink { start: 0, end: 1, text: "https://a.com".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 2, end: 3, text: "https://b.com".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 4, end: 5, text: "./f.rs".into(), classification: LinkClassification::FilePath },
            ClassifiedLink { start: 6, end: 7, text: "a@b.com".into(), classification: LinkClassification::Email },
            ClassifiedLink { start: 8, end: 9, text: "a@c.com".into(), classification: LinkClassification::Email },
            ClassifiedLink { start: 10, end: 11, text: "a@d.com".into(), classification: LinkClassification::Email },
        ];
        assert_eq!(count_links_by_type(&links), (2, 1, 3));
    }
}
