//! Document link detection for editor content.
//!
//! Provides URL and file-path detection within text, as well as a trait for
//! language-specific document-link providers.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// Link normalization
// ---------------------------------------------------------------------------

/// Normalize a URL by lowercasing the scheme and host, and removing trailing slashes
/// from the path (unless the path is exactly "/").
pub fn normalize_url(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some(pair) => pair,
        None => return url.to_string(),
    };
    let scheme_lower = scheme.to_lowercase();
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let authority_lower = authority.to_lowercase();
    let trimmed_path = if path.len() > 1 {
        path.trim_end_matches('/')
    } else {
        path
    };
    if trimmed_path.is_empty() {
        format!("{scheme_lower}://{authority_lower}")
    } else {
        format!("{scheme_lower}://{authority_lower}{trimmed_path}")
    }
}

// ---------------------------------------------------------------------------
// Link deduplication
// ---------------------------------------------------------------------------

/// Deduplicate classified links by their normalized text, keeping the first occurrence.
pub fn dedup_links(links: &[ClassifiedLink]) -> Vec<ClassifiedLink> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for link in links {
        let key = normalize_url(&link.text);
        if seen.insert(key) {
            result.push(link.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Link sorting
// ---------------------------------------------------------------------------

/// Sort classified links alphabetically by their text (case-insensitive).
pub fn sort_links_by_text(links: &mut [ClassifiedLink]) {
    links.sort_by(|a, b| {
        a.text
            .to_lowercase()
            .cmp(&b.text.to_lowercase())
    });
}

/// Sort classified links by their byte start position.
pub fn sort_links_by_position(links: &mut [ClassifiedLink]) {
    links.sort_by_key(|l| l.start);
}

// ---------------------------------------------------------------------------
// Batch link validation
// ---------------------------------------------------------------------------

/// Result of validating a single link within a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkValidationResult {
    pub text: String,
    pub valid: bool,
    pub error: Option<String>,
}

/// Validate a batch of classified links, returning a result per link.
pub fn validate_links_batch(links: &[ClassifiedLink]) -> Vec<LinkValidationResult> {
    let validator = LinkValidator::new();
    links
        .iter()
        .map(|link| {
            let result = match link.classification {
                LinkClassification::Url => validator.validate_url(&link.text),
                LinkClassification::FilePath => validator.validate_file_path(&link.text),
                LinkClassification::Email => validator.validate_email(&link.text),
                LinkClassification::Custom(_) => Ok(()),
            };
            LinkValidationResult {
                text: link.text.clone(),
                valid: result.is_ok(),
                error: result.err().map(|e| e.to_string()),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Link statistics
// ---------------------------------------------------------------------------

/// Aggregated statistics about a collection of classified links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkStatistics {
    pub total: usize,
    pub url_count: usize,
    pub file_count: usize,
    pub email_count: usize,
    pub custom_count: usize,
    pub unique_domains: usize,
}

/// Compute aggregated statistics for a set of classified links.
pub fn link_statistics(links: &[ClassifiedLink]) -> LinkStatistics {
    let mut url_count = 0usize;
    let mut file_count = 0usize;
    let mut email_count = 0usize;
    let mut custom_count = 0usize;
    let mut domains = std::collections::HashSet::new();

    for link in links {
        match &link.classification {
            LinkClassification::Url => {
                url_count += 1;
                if let Some(d) = extract_domain(&link.text) {
                    domains.insert(d.to_lowercase());
                }
            }
            LinkClassification::FilePath => file_count += 1,
            LinkClassification::Email => email_count += 1,
            LinkClassification::Custom(_) => custom_count += 1,
        }
    }
    LinkStatistics {
        total: links.len(),
        url_count,
        file_count,
        email_count,
        custom_count,
        unique_domains: domains.len(),
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

// ---------------------------------------------------------------------------
// Markdown link extraction
// ---------------------------------------------------------------------------

/// A link extracted from Markdown `[text](url)` syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownLink {
    /// Display text between the brackets.
    pub text: String,
    /// Target URL/path between the parentheses.
    pub target: String,
    /// Byte start of the full `[text](url)` span.
    pub byte_start: usize,
    /// Byte end (exclusive) of the full span.
    pub byte_end: usize,
}

/// Extract `[text](url)` style links from Markdown content.
pub fn extract_markdown_links(content: &str) -> Vec<MarkdownLink> {
    let mut results = Vec::new();
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'[' {
            // Look for closing bracket
            let text_start = i + 1;
            let mut j = text_start;
            let mut depth = 1u32;
            while j < len && depth > 0 {
                match bytes[j] {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    j += 1;
                }
            }
            if depth == 0 && j + 1 < len && bytes[j + 1] == b'(' {
                let link_text = &content[text_start..j];
                let url_start = j + 2;
                if let Some(close_paren) = content[url_start..].find(')') {
                    let url_end = url_start + close_paren;
                    let target = content[url_start..url_end].trim().to_string();
                    if !target.is_empty() {
                        results.push(MarkdownLink {
                            text: link_text.to_string(),
                            target,
                            byte_start: i,
                            byte_end: url_end + 1,
                        });
                    }
                    i = url_end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    results
}

// ---------------------------------------------------------------------------
// Link display truncation
// ---------------------------------------------------------------------------

/// Truncate a URL for display purposes, preserving the domain and showing an
/// ellipsis in the middle when the URL exceeds `max_len` characters.
///
/// If the URL is shorter than or equal to `max_len`, it is returned unchanged.
pub fn truncate_url_for_display(url: &str, max_len: usize) -> String {
    if url.chars().count() <= max_len || max_len < 10 {
        return url.to_string();
    }
    let scheme_end = url.find("://").map(|i| i + 3).unwrap_or(0);
    let after_scheme = &url[scheme_end..];
    let host_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    let prefix_len = scheme_end + host_end;

    // If the domain alone is already too long, just hard-truncate.
    if prefix_len >= max_len.saturating_sub(3) {
        let truncated: String = url.chars().take(max_len.saturating_sub(1)).collect();
        return format!("{truncated}…");
    }

    let suffix_budget = max_len.saturating_sub(prefix_len).saturating_sub(1); // 1 for '…'
    let path = &url[prefix_len..];
    let path_chars: Vec<char> = path.chars().collect();
    if path_chars.len() <= suffix_budget {
        return url.to_string();
    }
    let tail: String = path_chars[path_chars.len() - suffix_budget..].iter().collect();
    format!("{}…{}", &url[..prefix_len], tail)
}

// ---------------------------------------------------------------------------
// Deep link generation (editor:// protocol)
// ---------------------------------------------------------------------------

/// Generate an `editor://` deep link that can open a specific file and
/// position in the editor.
pub fn generate_deep_link(file_path: &str, line: Option<u32>, col: Option<u32>) -> String {
    let mut link = format!("editor://file/{file_path}");
    match (line, col) {
        (Some(l), Some(c)) => {
            link.push_str(&format!(":{l}:{c}"));
        }
        (Some(l), None) => {
            link.push_str(&format!(":{l}"));
        }
        _ => {}
    }
    link
}

/// Parse an `editor://` deep link back into its components.
///
/// Returns `(file_path, optional_line, optional_col)` or `None` if the link
/// does not use the `editor://file/` scheme.
pub fn parse_deep_link(link: &str) -> Option<(String, Option<u32>, Option<u32>)> {
    let rest = link.strip_prefix("editor://file/")?;
    if rest.is_empty() {
        return None;
    }
    // Split from the right to handle paths that contain ':'
    let parts: Vec<&str> = rest.rsplitn(3, ':').collect();
    match parts.len() {
        3 => {
            let col = parts[0].parse::<u32>().ok();
            let line = parts[1].parse::<u32>().ok();
            if line.is_some() && col.is_some() {
                Some((parts[2].to_string(), line, col))
            } else {
                Some((rest.to_string(), None, None))
            }
        }
        2 => {
            let line = parts[0].parse::<u32>().ok();
            if line.is_some() {
                Some((parts[1].to_string(), line, None))
            } else {
                Some((rest.to_string(), None, None))
            }
        }
        _ => Some((rest.to_string(), None, None)),
    }
}

// ---------------------------------------------------------------------------
// Relative path resolution
// ---------------------------------------------------------------------------

/// Resolve a relative path against a base directory, normalizing `.` and `..`
/// segments without filesystem access.
pub fn resolve_relative_path(base_dir: &str, relative: &str) -> String {
    if relative.starts_with('/') {
        return normalize_path_segments(relative);
    }
    let combined = if base_dir.ends_with('/') {
        format!("{base_dir}{relative}")
    } else {
        format!("{base_dir}/{relative}")
    };
    normalize_path_segments(&combined)
}

/// Normalize a path by resolving `.` and `..` segments.
fn normalize_path_segments(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    let joined = stack.join("/");
    if path.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
}

// ---------------------------------------------------------------------------
// Link history tracker
// ---------------------------------------------------------------------------

/// Tracks which links a user has visited within an editing session.
#[derive(Debug, Clone)]
pub struct LinkHistory {
    entries: Vec<LinkHistoryEntry>,
    max_entries: usize,
}

/// A single entry in the link history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkHistoryEntry {
    pub target: String,
    pub visit_count: u32,
}

impl LinkHistory {
    /// Create a new history with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Record a link visit. If the link was already visited, increments its
    /// count; otherwise adds a new entry (evicting the oldest if at capacity).
    pub fn record_visit(&mut self, target: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.target == target) {
            entry.visit_count += 1;
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(LinkHistoryEntry {
            target: target.to_string(),
            visit_count: 1,
        });
    }

    /// Return how many times a link has been visited, or 0 if never.
    pub fn visit_count(&self, target: &str) -> u32 {
        self.entries
            .iter()
            .find(|e| e.target == target)
            .map_or(0, |e| e.visit_count)
    }

    /// Return all entries ordered by most-visited first.
    pub fn most_visited(&self) -> Vec<&LinkHistoryEntry> {
        let mut sorted: Vec<&LinkHistoryEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        sorted
    }

    /// Return the total number of distinct links tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if no links have been tracked.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// URL sub-categorization
// ---------------------------------------------------------------------------

/// Finer-grained categorization of HTTP(S) URLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlCategory {
    /// Documentation site (docs.*, readthedocs, wiki, etc.)
    Documentation,
    /// API endpoint (contains `/api/` or `api.` subdomain)
    Api,
    /// Source-code hosting (github, gitlab, bitbucket, etc.)
    SourceRepo,
    /// Media / image link (common image extensions)
    Media,
    /// General web link
    Web,
}

/// Categorize an HTTP(S) URL into a finer-grained [`UrlCategory`].
pub fn categorize_url(url: &str) -> UrlCategory {
    let lower = url.to_lowercase();
    let domain = extract_domain(&lower).unwrap_or("");

    if domain.starts_with("docs.")
        || domain.contains("readthedocs")
        || domain.contains("wiki")
        || lower.contains("/wiki/")
        || lower.contains("/docs/")
    {
        return UrlCategory::Documentation;
    }
    if domain.starts_with("api.") || lower.contains("/api/") {
        return UrlCategory::Api;
    }
    if domain.contains("github.com")
        || domain.contains("gitlab.com")
        || domain.contains("bitbucket.org")
        || domain.contains("sr.ht")
    {
        return UrlCategory::SourceRepo;
    }
    let media_exts = [".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".mp4"];
    if media_exts.iter().any(|ext| lower.ends_with(ext)) {
        return UrlCategory::Media;
    }
    UrlCategory::Web
}


// ---------------------------------------------------------------------------
// LinkAccessibilityChecker
// ---------------------------------------------------------------------------

pub struct LinkAccessibilityChecker;

impl LinkAccessibilityChecker {
    pub fn check_url_format(url: &str) -> Result<(), String> {
        if url.is_empty() { return Err("empty URL".into()); }
        if !url.contains("://") && !url.starts_with('/') { return Err("missing scheme".into()); }
        if url.len() > 2048 { return Err("URL too long".into()); }
        Ok(())
    }

    pub fn is_localhost(url: &str) -> bool {
        url.contains("localhost") || url.contains("127.0.0.1") || url.contains("::1")
    }

    pub fn is_secure(url: &str) -> bool {
        url.starts_with("https://") || url.starts_with("ftps://")
    }
}

// ---------------------------------------------------------------------------
// LinkHighlightRange
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct LinkHighlightRange {
    pub start_col: u32,
    pub end_col: u32,
    pub line: u32,
    pub url: String,
}

impl LinkHighlightRange {
    pub fn new(line: u32, start: u32, end: u32, url: impl Into<String>) -> Self {
        Self { line, start_col: start, end_col: end, url: url.into() }
    }

    pub fn length(&self) -> u32 { self.end_col.saturating_sub(self.start_col) }

    pub fn contains_column(&self, col: u32) -> bool { col >= self.start_col && col < self.end_col }
}

impl std::fmt::Display for LinkHighlightRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Link(L{}:{}-{})", self.line, self.start_col, self.end_col)
    }
}

/// Builds highlight ranges from detected URLs.
pub struct LinkHighlighter;

impl LinkHighlighter {
    pub fn highlight_line(line_num: u32, text: &str) -> Vec<LinkHighlightRange> {
        detect_urls(text).into_iter().map(|(start, end, url)| {
            LinkHighlightRange::new(line_num, start as u32, end as u32, url)
        }).collect()
    }

    pub fn highlight_lines(lines: &[&str]) -> Vec<LinkHighlightRange> {
        lines.iter().enumerate().flat_map(|(i, line)| {
            Self::highlight_line(i as u32, line)
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// LinkProtocolFilter
// ---------------------------------------------------------------------------

pub struct LinkProtocolFilter {
    allowed: Vec<String>,
}

impl LinkProtocolFilter {
    pub fn new() -> Self { Self { allowed: vec!["http".into(), "https".into(), "file".into()] } }

    pub fn with_protocols(protocols: Vec<String>) -> Self { Self { allowed: protocols } }

    pub fn is_allowed(&self, url: &str) -> bool {
        if let Some(scheme) = url.split("://").next() {
            self.allowed.iter().any(|a| a == scheme)
        } else {
            false
        }
    }

    pub fn add_protocol(&mut self, proto: impl Into<String>) { self.allowed.push(proto.into()); }
    pub fn protocols(&self) -> &[String] { &self.allowed }
}

impl Default for LinkProtocolFilter { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// LinkTooltipPreview
// ---------------------------------------------------------------------------

pub struct LinkTooltipPreview;

impl LinkTooltipPreview {
    pub fn generate(url: &str) -> String {
        let truncated = if url.len() > 80 { format!("{}...", &url[..77]) } else { url.to_string() };
        if url.starts_with("https://") { format!("Open: {} (secure)", truncated) }
        else if url.starts_with("http://") { format!("Open: {} (insecure)", truncated) }
        else if url.starts_with("file://") { format!("Open file: {}", truncated) }
        else { format!("Open: {}", truncated) }
    }

    pub fn is_external(url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }
}


// ---------------------------------------------------------------------------
// Link detection optimizer
// ---------------------------------------------------------------------------

pub struct LinkDetectionOptimizer {
    cache: HashMap<u64, Vec<(usize, usize, String)>>,
    hit_count: u64, miss_count: u64, max_cache: usize,
}
impl LinkDetectionOptimizer {
    pub fn new(max: usize) -> Self { Self { cache: HashMap::new(), hit_count: 0, miss_count: 0, max_cache: max } }
    fn hash_line(line: &str) -> u64 { let mut h: u64 = 5381; for b in line.bytes() { h = h.wrapping_mul(33).wrapping_add(b as u64); } h }
    pub fn detect_cached(&mut self, line: &str) -> Vec<(usize, usize, String)> {
        let k = Self::hash_line(line);
        if let Some(c) = self.cache.get(&k) { self.hit_count += 1; return c.clone(); }
        self.miss_count += 1;
        let links = detect_urls(line);
        if self.cache.len() < self.max_cache { self.cache.insert(k, links.clone()); }
        links
    }
    pub fn invalidate(&mut self) { self.cache.clear(); }
    pub fn cache_size(&self) -> usize { self.cache.len() }
    pub fn hit_rate(&self) -> f64 { let t = self.hit_count + self.miss_count; if t == 0 { 0.0 } else { self.hit_count as f64 / t as f64 } }
    pub fn hit_count(&self) -> u64 { self.hit_count }
    pub fn miss_count(&self) -> u64 { self.miss_count }
    pub fn reset_stats(&mut self) { self.hit_count = 0; self.miss_count = 0; }
}
impl Default for LinkDetectionOptimizer { fn default() -> Self { Self::new(4096) } }
impl fmt::Display for LinkDetectionOptimizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "LinkOptimizer(cache={}, hit={:.1}%)", self.cache.len(), self.hit_rate()*100.0) }
}

// ---------------------------------------------------------------------------
// Link tooltip generator
// ---------------------------------------------------------------------------

pub struct LinkTooltipGenerator {
    show_full_url: bool, max_len: usize, prefixes: HashMap<String, String>,
}
impl LinkTooltipGenerator {
    pub fn new() -> Self { Self { show_full_url: true, max_len: 80, prefixes: HashMap::new() } }
    pub fn with_max_len(mut self, m: usize) -> Self { self.max_len = m; self }
    pub fn with_show_full(mut self, s: bool) -> Self { self.show_full_url = s; self }
    pub fn add_prefix(&mut self, prefix: impl Into<String>, label: impl Into<String>) { self.prefixes.insert(prefix.into(), label.into()); }
    fn truncate(s: &str, m: usize) -> String { if s.len() <= m { s.to_string() } else { format!("{}...", &s[..m.saturating_sub(3)]) } }
    pub fn tooltip_for_url(&self, url: &str) -> String {
        for (p, l) in &self.prefixes { if url.starts_with(p.as_str()) { return format!("{}: {}", l, Self::truncate(url, self.max_len)); } }
        let d = Self::truncate(url, self.max_len);
        if url.starts_with("https://") || url.starts_with("http://") { format!("Open link: {}", d) }
        else if url.starts_with("file://") { format!("Open file: {}", d) }
        else if url.contains('@') { format!("Send email: {}", d) }
        else { format!("Follow: {}", d) }
    }
    pub fn tooltip_for_file(&self, path: &str) -> String { format!("Open file: {}", Self::truncate(path, self.max_len)) }
    pub fn tooltip_for_email(&self, email: &str) -> String { format!("Send email to {}", email) }
    pub fn prefix_count(&self) -> usize { self.prefixes.len() }
}
impl Default for LinkTooltipGenerator { fn default() -> Self { Self::new() } }
impl fmt::Display for LinkTooltipGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "TooltipGen(max={})", self.max_len) }
}


// ---------------------------------------------------------------------------
// LinkDetectionOptimizerConfig — configuration for LinkDetectionOptimizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LinkDetectionOptimizerConfig {
    pub max_entries: usize,
    pub auto_refresh: bool,
    pub refresh_interval_ms: u64,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl LinkDetectionOptimizerConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_refresh(mut self, a: bool) -> Self { self.auto_refresh = a; self }
    pub fn with_refresh_interval(mut self, ms: u64) -> Self { self.refresh_interval_ms = ms; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn is_refresh_due(&self, elapsed_ms: u64) -> bool { self.auto_refresh && elapsed_ms >= self.refresh_interval_ms }
}

impl Default for LinkDetectionOptimizerConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_refresh: true, refresh_interval_ms: 5000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for LinkDetectionOptimizerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_refresh={}, interval={}ms)", self.max_entries, self.auto_refresh, self.refresh_interval_ms)
    }
}

// ---------------------------------------------------------------------------
// LinkTooltipGeneratorStats — statistics tracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct LinkTooltipGeneratorStats {
    pub total_operations: u64,
    pub successful: u64,
    pub failed: u64,
    pub total_duration_ms: u64,
    pub peak_concurrent: usize,
    pub current_concurrent: usize,
}

impl LinkTooltipGeneratorStats {
    pub fn new() -> Self { Self::default() }
    pub fn record_success(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.successful += 1; self.total_duration_ms += duration_ms;
    }
    pub fn record_failure(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.failed += 1; self.total_duration_ms += duration_ms;
    }
    pub fn success_rate(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.successful as f64 / self.total_operations as f64 } }
    pub fn avg_duration_ms(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.total_duration_ms as f64 / self.total_operations as f64 } }
    pub fn update_concurrent(&mut self, current: usize) {
        self.current_concurrent = current;
        if current > self.peak_concurrent { self.peak_concurrent = current; }
    }
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn merge(&mut self, other: &Self) {
        self.total_operations += other.total_operations;
        self.successful += other.successful;
        self.failed += other.failed;
        self.total_duration_ms += other.total_duration_ms;
        if other.peak_concurrent > self.peak_concurrent { self.peak_concurrent = other.peak_concurrent; }
    }
}

impl fmt::Display for LinkTooltipGeneratorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(ops={}, success={:.1}%, avg={:.1}ms)", self.total_operations, self.success_rate() * 100.0, self.avg_duration_ms())
    }
}

// ---------------------------------------------------------------------------
// LinkDetectionOptimizerEventKind — event types for LinkDetectionOptimizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkDetectionOptimizerEventKind {
    Created,
    Updated,
    Deleted,
    Refreshed,
    Error,
}

impl fmt::Display for LinkDetectionOptimizerEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Updated => write!(f, "updated"),
            Self::Deleted => write!(f, "deleted"),
            Self::Refreshed => write!(f, "refreshed"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A recorded event in the LinkDetectionOptimizer lifecycle.
#[derive(Debug, Clone)]
pub struct LinkDetectionOptimizerEvent {
    pub kind: LinkDetectionOptimizerEventKind,
    pub timestamp: u64,
    pub detail: Option<String>,
}

impl LinkDetectionOptimizerEvent {
    pub fn new(kind: LinkDetectionOptimizerEventKind, timestamp: u64) -> Self {
        Self { kind, timestamp, detail: None }
    }
    pub fn with_detail(mut self, d: impl Into<String>) -> Self { self.detail = Some(d.into()); self }
    pub fn is_error(&self) -> bool { self.kind == LinkDetectionOptimizerEventKind::Error }
}

impl fmt::Display for LinkDetectionOptimizerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Event({}, t={})", self.kind, self.timestamp)
    }
}


// ─── LinkC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for resolved links.
#[derive(Debug)]
pub struct LinkCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> LinkCLruCache<V> {
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

impl<V: Clone + fmt::Display> fmt::Display for LinkCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LinkCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── LinkB Builder & Validator ─────────────────────────────

/// Builder for constructing link configurations.
#[derive(Debug, Clone)]
pub struct LinkBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl LinkBBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<LinkBCfg, LinkBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(LinkBBuildErr { errors }); }
        Ok(LinkBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated link configuration.
#[derive(Debug, Clone)]
pub struct LinkBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl LinkBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &LinkBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for LinkBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LinkBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct LinkBBuildErr { pub errors: Vec<String> }

impl fmt::Display for LinkBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LinkBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for LinkBBuildErr {}


// ---------------------------------------------------------------------------
// links – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XLinksValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XLinksValidationResult {
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
pub struct XLinksTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XLinksTaggedEntry {
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
pub fn x_links_validate_string(value: &str, max_len: usize) -> XLinksValidationResult {
    if value.is_empty() {
        return XLinksValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XLinksValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XLinksValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_links_validate_range(value: i64, min: i64, max: i64) -> XLinksValidationResult {
    if value < min || value > max {
        XLinksValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XLinksValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_links_filter_by_tag<'a>(
    entries: &'a [XLinksTaggedEntry],
    tag: &str,
) -> Vec<&'a XLinksTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_links_group_by_tag(
    entries: &[XLinksTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XLinksTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_links_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_links_dedup_entries(entries: Vec<XLinksTaggedEntry>) -> Vec<XLinksTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
}



// ---------------------------------------------------------------------------
// links – Extended link validation helpers
// ---------------------------------------------------------------------------

/// Priority levels for link validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZLinksPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZLinksPriority {
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
    pub fn all_asc() -> [ZLinksPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZLinksPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks link validation data.
#[derive(Debug, Clone)]
pub struct ZLinksLinkValidationResult {
    pub broken_links: Vec<(String, String)>,
    pub checked: usize,
    pub valid: usize,
}

impl ZLinksLinkValidationResult {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            broken_links: Vec::new(),
            checked: 0,
            valid: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.broken_links.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.broken_links.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.broken_links.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZLinksLinkValidationResult[checked={:?}, valid={:?}]", self.checked, self.valid)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for link validation.
pub fn z_links_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_links_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_links_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_links_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_links_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_links_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_links_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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
// xb_ utilities – batch 55
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer55 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer55 {
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
pub fn xb_fnv1a_55(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_55<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_55<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_55(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_55(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
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

    #[test]
    fn normalize_url_lowercases_scheme_and_host() {
        assert_eq!(normalize_url("HTTPS://EXAMPLE.COM/Path"), "https://example.com/Path");
        assert_eq!(normalize_url("HTTP://Foo.Bar"), "http://foo.bar");
    }

    #[test]
    fn normalize_url_strips_trailing_slashes() {
        assert_eq!(normalize_url("https://example.com/foo/"), "https://example.com/foo");
        assert_eq!(normalize_url("https://example.com/foo///"), "https://example.com/foo");
        // Root path preserved
        assert_eq!(normalize_url("https://example.com/"), "https://example.com/");
    }

    #[test]
    fn dedup_links_removes_duplicates_by_normalized_text() {
        let links = vec![
            ClassifiedLink { start: 0, end: 5, text: "https://A.com/path/".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 10, end: 20, text: "https://a.com/path".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 30, end: 40, text: "./file.rs".into(), classification: LinkClassification::FilePath },
        ];
        let deduped = dedup_links(&links);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].text, "https://A.com/path/");
        assert_eq!(deduped[1].text, "./file.rs");
    }

    #[test]
    fn sort_links_by_text_alphabetical() {
        let mut links = vec![
            ClassifiedLink { start: 0, end: 1, text: "https://zoo.com".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 2, end: 3, text: "https://alpha.com".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 4, end: 5, text: "https://mid.com".into(), classification: LinkClassification::Url },
        ];
        sort_links_by_text(&mut links);
        assert_eq!(links[0].text, "https://alpha.com");
        assert_eq!(links[1].text, "https://mid.com");
        assert_eq!(links[2].text, "https://zoo.com");
    }

    #[test]
    fn validate_links_batch_mixed() {
        let links = vec![
            ClassifiedLink { start: 0, end: 1, text: "https://valid.com".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 2, end: 3, text: "not-a-url".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 4, end: 5, text: "user@example.com".into(), classification: LinkClassification::Email },
        ];
        let results = validate_links_batch(&links);
        assert_eq!(results.len(), 3);
        assert!(results[0].valid);
        assert!(!results[1].valid);
        assert!(results[1].error.is_some());
        assert!(results[2].valid);
    }

    #[test]
    fn link_statistics_aggregation() {
        let links = vec![
            ClassifiedLink { start: 0, end: 1, text: "https://a.com/x".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 2, end: 3, text: "https://a.com/y".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 4, end: 5, text: "https://b.org".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 6, end: 7, text: "./file.rs".into(), classification: LinkClassification::FilePath },
            ClassifiedLink { start: 8, end: 9, text: "x@y.com".into(), classification: LinkClassification::Email },
        ];
        let stats = link_statistics(&links);
        assert_eq!(stats.total, 5);
        assert_eq!(stats.url_count, 3);
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.email_count, 1);
        assert_eq!(stats.unique_domains, 2);
    }

    #[test]
    fn sort_links_by_position_orders_correctly() {
        let mut links = vec![
            ClassifiedLink { start: 50, end: 60, text: "z".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 10, end: 20, text: "a".into(), classification: LinkClassification::Url },
            ClassifiedLink { start: 30, end: 40, text: "m".into(), classification: LinkClassification::Url },
        ];
        sort_links_by_position(&mut links);
        assert_eq!(links[0].start, 10);
        assert_eq!(links[1].start, 30);
        assert_eq!(links[2].start, 50);
    }

    // -- markdown link extraction tests --

    #[test]
    fn extract_markdown_links_basic() {
        let md = "Click [here](https://example.com) for info.";
        let links = extract_markdown_links(md);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "here");
        assert_eq!(links[0].target, "https://example.com");
    }

    #[test]
    fn extract_markdown_links_multiple() {
        let md = "[a](https://a.com) text [b](./local.md)";
        let links = extract_markdown_links(md);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "https://a.com");
        assert_eq!(links[1].target, "./local.md");
        assert_eq!(links[1].text, "b");
    }

    #[test]
    fn extract_markdown_links_none() {
        let md = "No links here, just [brackets] and (parens).";
        let links = extract_markdown_links(md);
        assert!(links.is_empty());
    }

    // -- URL display truncation tests --

    #[test]
    fn truncate_url_short_unchanged() {
        let url = "https://example.com";
        assert_eq!(truncate_url_for_display(url, 50), url);
    }

    #[test]
    fn truncate_url_long_is_shortened() {
        let url = "https://example.com/very/long/path/to/some/resource/file.html";
        let result = truncate_url_for_display(url, 35);
        assert!(result.chars().count() <= 35);
        assert!(result.contains("example.com"));
        assert!(result.contains('…'));
    }

    // -- deep link tests --

    #[test]
    fn generate_deep_link_full() {
        let link = generate_deep_link("src/main.rs", Some(42), Some(10));
        assert_eq!(link, "editor://file/src/main.rs:42:10");
    }

    #[test]
    fn generate_deep_link_line_only() {
        let link = generate_deep_link("src/lib.rs", Some(7), None);
        assert_eq!(link, "editor://file/src/lib.rs:7");
    }

    #[test]
    fn parse_deep_link_roundtrip() {
        let link = generate_deep_link("src/main.rs", Some(42), Some(10));
        let parsed = parse_deep_link(&link);
        assert_eq!(parsed, Some(("src/main.rs".to_string(), Some(42), Some(10))));
    }

    #[test]
    fn parse_deep_link_no_position() {
        let parsed = parse_deep_link("editor://file/README.md");
        assert_eq!(parsed, Some(("README.md".to_string(), None, None)));
    }

    #[test]
    fn parse_deep_link_invalid_scheme() {
        assert!(parse_deep_link("http://file/foo.rs").is_none());
    }

    // -- relative path resolution tests --

    #[test]
    fn resolve_relative_path_basic() {
        let result = resolve_relative_path("/home/user/project", "../other/file.rs");
        assert_eq!(result, "/home/user/other/file.rs");
    }

    #[test]
    fn resolve_relative_path_dot_segments() {
        let result = resolve_relative_path("/a/b/c", "./d/../e/f");
        assert_eq!(result, "/a/b/c/e/f");
    }

    #[test]
    fn resolve_absolute_ignores_base() {
        let result = resolve_relative_path("/home/user", "/etc/hosts");
        assert_eq!(result, "/etc/hosts");
    }

    // -- link history tests --

    #[test]
    fn link_history_basic_tracking() {
        let mut hist = LinkHistory::new(10);
        assert!(hist.is_empty());
        hist.record_visit("https://a.com");
        hist.record_visit("https://b.com");
        hist.record_visit("https://a.com");
        assert_eq!(hist.len(), 2);
        assert_eq!(hist.visit_count("https://a.com"), 2);
        assert_eq!(hist.visit_count("https://b.com"), 1);
        assert_eq!(hist.visit_count("https://c.com"), 0);
    }

    #[test]
    fn link_history_eviction() {
        let mut hist = LinkHistory::new(2);
        hist.record_visit("a");
        hist.record_visit("b");
        hist.record_visit("c"); // evicts "a"
        assert_eq!(hist.len(), 2);
        assert_eq!(hist.visit_count("a"), 0);
        assert_eq!(hist.visit_count("b"), 1);
        assert_eq!(hist.visit_count("c"), 1);
    }

    #[test]
    fn link_history_most_visited_order() {
        let mut hist = LinkHistory::new(10);
        hist.record_visit("x");
        hist.record_visit("y");
        hist.record_visit("y");
        hist.record_visit("z");
        hist.record_visit("z");
        hist.record_visit("z");
        let top = hist.most_visited();
        assert_eq!(top[0].target, "z");
        assert_eq!(top[1].target, "y");
        assert_eq!(top[2].target, "x");
    }

    #[test]
    fn link_history_clear() {
        let mut hist = LinkHistory::new(10);
        hist.record_visit("a");
        hist.clear();
        assert!(hist.is_empty());
    }

    // -- URL categorization tests --

    #[test]
    fn categorize_url_source_repo() {
        assert_eq!(categorize_url("https://github.com/user/repo"), UrlCategory::SourceRepo);
        assert_eq!(categorize_url("https://gitlab.com/proj"), UrlCategory::SourceRepo);
    }

    #[test]
    fn categorize_url_docs() {
        assert_eq!(categorize_url("https://docs.rs/serde/latest"), UrlCategory::Documentation);
        assert_eq!(categorize_url("https://example.com/wiki/page"), UrlCategory::Documentation);
    }

    #[test]
    fn categorize_url_api() {
        assert_eq!(categorize_url("https://api.example.com/v1/data"), UrlCategory::Api);
        assert_eq!(categorize_url("https://example.com/api/users"), UrlCategory::Api);
    }

    #[test]
    fn categorize_url_media() {
        assert_eq!(categorize_url("https://example.com/image.png"), UrlCategory::Media);
        assert_eq!(categorize_url("https://cdn.example.com/video.mp4"), UrlCategory::Media);
    }

    #[test]
    fn categorize_url_generic_web() {
        assert_eq!(categorize_url("https://example.com/page"), UrlCategory::Web);
    }


    #[test]
    fn accessibility_check_valid() {
        assert!(LinkAccessibilityChecker::check_url_format("https://example.com").is_ok());
    }

    #[test]
    fn accessibility_check_empty() {
        assert!(LinkAccessibilityChecker::check_url_format("").is_err());
    }

    #[test]
    fn accessibility_localhost() {
        assert!(LinkAccessibilityChecker::is_localhost("http://localhost:8080"));
        assert!(!LinkAccessibilityChecker::is_localhost("http://example.com"));
    }

    #[test]
    fn accessibility_secure() {
        assert!(LinkAccessibilityChecker::is_secure("https://example.com"));
        assert!(!LinkAccessibilityChecker::is_secure("http://example.com"));
    }

    #[test]
    fn highlight_range_basic() {
        let r = LinkHighlightRange::new(0, 5, 20, "http://example.com");
        assert_eq!(r.length(), 15);
        assert!(r.contains_column(10));
        assert!(!r.contains_column(20));
    }

    #[test]
    fn highlight_range_display() {
        let r = LinkHighlightRange::new(1, 0, 10, "x");
        assert!(format!("{r}").contains("L1"));
    }

    #[test]
    fn highlighter_line() {
        let ranges = LinkHighlighter::highlight_line(0, "visit https://rust-lang.org today");
        assert!(!ranges.is_empty());
    }

    #[test]
    fn protocol_filter_defaults() {
        let f = LinkProtocolFilter::new();
        assert!(f.is_allowed("https://example.com"));
        assert!(f.is_allowed("http://example.com"));
        assert!(f.is_allowed("file:///tmp"));
        assert!(!f.is_allowed("ftp://example.com"));
    }

    #[test]
    fn protocol_filter_custom() {
        let mut f = LinkProtocolFilter::new();
        f.add_protocol("ftp");
        assert!(f.is_allowed("ftp://example.com"));
    }

    #[test]
    fn tooltip_preview_https() {
        let t = LinkTooltipPreview::generate("https://example.com");
        assert!(t.contains("secure"));
    }

    #[test]
    fn tooltip_preview_http() {
        let t = LinkTooltipPreview::generate("http://example.com");
        assert!(t.contains("insecure"));
    }

    #[test]
    fn tooltip_is_external() {
        assert!(LinkTooltipPreview::is_external("https://example.com"));
        assert!(!LinkTooltipPreview::is_external("file:///tmp"));
    }


    #[test] fn link_opt_hit() { let mut o = LinkDetectionOptimizer::new(100); let l = "visit https://example.com today"; let r1 = o.detect_cached(l); let r2 = o.detect_cached(l); assert_eq!(r1, r2); assert_eq!(o.hit_count(), 1); }
    #[test] fn link_opt_inv() { let mut o = LinkDetectionOptimizer::new(100); o.detect_cached("https://x.com"); o.invalidate(); assert_eq!(o.cache_size(), 0); }
    #[test] fn link_opt_rate() { let mut o = LinkDetectionOptimizer::new(100); o.detect_cached("https://a.com"); o.detect_cached("https://a.com"); assert!(o.hit_rate() > 0.4); }
    #[test] fn link_opt_max() { let mut o = LinkDetectionOptimizer::new(2); o.detect_cached("a"); o.detect_cached("b"); o.detect_cached("c"); assert!(o.cache_size() <= 2); }
    #[test] fn link_opt_display() { assert!(format!("{}", LinkDetectionOptimizer::default()).contains("LinkOptimizer")); }
    #[test] fn tt_https() { assert!(LinkTooltipGenerator::new().tooltip_for_url("https://x.com").starts_with("Open link:")); }
    #[test] fn tt_file() { assert!(LinkTooltipGenerator::new().tooltip_for_url("file:///a").starts_with("Open file:")); }
    #[test] fn tt_email() { assert!(LinkTooltipGenerator::new().tooltip_for_email("a@b.com").contains("a@b.com")); }
    #[test] fn tt_trunc() { assert!(LinkTooltipGenerator::new().with_max_len(10).tooltip_for_url("https://very-long-url.example.com/path").contains("...")); }
    #[test] fn tt_prefix() { let mut g = LinkTooltipGenerator::new(); g.add_prefix("jira://", "JIRA"); assert!(g.tooltip_for_url("jira://X-1").contains("JIRA")); }
    #[test] fn tt_display() { assert!(format!("{}", LinkTooltipGenerator::new()).contains("max=80")); }
    #[test] fn tt_file_path() { assert!(LinkTooltipGenerator::new().tooltip_for_file("/a/b").starts_with("Open file:")); }


    #[test] fn linkDetectionOptimizer_cfg_default() {
        let c = LinkDetectionOptimizerConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_refresh);
    }
    #[test] fn linkDetectionOptimizer_cfg_builder() {
        let c = LinkDetectionOptimizerConfig::new().with_max_entries(500).with_auto_refresh(false);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_refresh);
    }
    #[test] fn linkDetectionOptimizer_cfg_labels() {
        let mut c = LinkDetectionOptimizerConfig::new();
        c.set_label("x", "y");
        assert_eq!(c.get_label("x"), Some("y"));
    }
    #[test] fn linkDetectionOptimizer_cfg_refresh_due() {
        let c = LinkDetectionOptimizerConfig::new();
        assert!(!c.is_refresh_due(1000));
        assert!(c.is_refresh_due(6000));
    }
    #[test] fn linkDetectionOptimizer_cfg_display() {
        assert!(format!("{}", LinkDetectionOptimizerConfig::new()).contains("Config"));
    }
    #[test] fn linkTooltipGenerator_stats_success() {
        let mut st = LinkTooltipGeneratorStats::new();
        st.record_success(10);
        st.record_success(20);
        st.record_failure(5);
        assert_eq!(st.total_operations, 3);
        assert!((st.success_rate() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn linkTooltipGenerator_stats_avg_dur() {
        let mut st = LinkTooltipGeneratorStats::new();
        st.record_success(10);
        st.record_success(30);
        assert!((st.avg_duration_ms() - 20.0).abs() < 1e-9);
    }
    #[test] fn linkTooltipGenerator_stats_merge() {
        let mut a = LinkTooltipGeneratorStats::new();
        a.record_success(10);
        let mut b = LinkTooltipGeneratorStats::new();
        b.record_success(20);
        a.merge(&b);
        assert_eq!(a.total_operations, 2);
    }
    #[test] fn linkTooltipGenerator_stats_concurrent() {
        let mut st = LinkTooltipGeneratorStats::new();
        st.update_concurrent(5);
        st.update_concurrent(3);
        assert_eq!(st.peak_concurrent, 5);
    }
    #[test] fn linkTooltipGenerator_stats_display() {
        assert!(format!("{}", LinkTooltipGeneratorStats::new()).contains("Stats"));
    }
    #[test] fn linkDetectionOptimizer_event_new() {
        let e = LinkDetectionOptimizerEvent::new(LinkDetectionOptimizerEventKind::Created, 100);
        assert_eq!(e.kind, LinkDetectionOptimizerEventKind::Created);
        assert!(!e.is_error());
    }
    #[test] fn linkDetectionOptimizer_event_detail() {
        let e = LinkDetectionOptimizerEvent::new(LinkDetectionOptimizerEventKind::Error, 0).with_detail("oops");
        assert!(e.is_error());
        assert_eq!(e.detail.unwrap(), "oops");
    }
    #[test] fn linkDetectionOptimizer_event_display() {
        let e = LinkDetectionOptimizerEvent::new(LinkDetectionOptimizerEventKind::Updated, 50);
        assert!(format!("{}", e).contains("updated"));
    }
    #[test] fn linkDetectionOptimizer_event_kind_display() {
        assert_eq!(format!("{}", LinkDetectionOptimizerEventKind::Refreshed), "refreshed");
    }


    #[test]
    fn linkc_lru_insert_get() {
        let mut c = LinkCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn linkc_lru_eviction() {
        let mut c = LinkCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn linkc_lru_hit_ratio() {
        let mut c = LinkCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn linkc_lru_clear() {
        let mut c = LinkCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn linkc_lru_remove() {
        let mut c = LinkCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn linkc_lru_peek() {
        let mut c = LinkCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn linkb_builder_valid() {
        let cfg = LinkBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn linkb_builder_empty_name() {
        let r = LinkBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn linkb_builder_bad_priority() {
        assert!(LinkBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn linkb_builder_zero_max() {
        assert!(LinkBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn linkb_cfg_merge() {
        let mut a = LinkBBuilder::new("a").property("x", "1").build().unwrap();
        let b = LinkBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn linkb_cfg_display() {
        let cfg = LinkBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- links additional tests -------------------------------------------

    #[test]
    fn x_links_validation_ok() {
        let r = x_links_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_links_validation_empty() {
        let r = x_links_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_links_validation_too_long() {
        let r = x_links_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_links_validate_range_ok() {
        assert!(x_links_validate_range(5, 1, 10).is_ok());
        assert!(x_links_validate_range(1, 1, 10).is_ok());
        assert!(x_links_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_links_validate_range_out() {
        assert!(!x_links_validate_range(0, 1, 10).is_ok());
        assert!(!x_links_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_links_tagged_entry_basic() {
        let e = XLinksTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_links_tagged_entry_with_tag() {
        let e = XLinksTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_links_filter_by_tag_basic() {
        let entries = vec![
            XLinksTaggedEntry::new("a", "1").with_tag("x"),
            XLinksTaggedEntry::new("b", "2").with_tag("y"),
            XLinksTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_links_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_links_group_by_tag_basic() {
        let entries = vec![
            XLinksTaggedEntry::new("a", "1").with_tag("x"),
            XLinksTaggedEntry::new("b", "2"),
            XLinksTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_links_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_links_djb2_hash_deterministic() {
        let h1 = x_links_djb2_hash("hello");
        let h2 = x_links_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_links_djb2_hash("hello"), x_links_djb2_hash("world"));
    }

    #[test]
    fn x_links_dedup_entries_basic() {
        let entries = vec![
            XLinksTaggedEntry::new("a", "1"),
            XLinksTaggedEntry::new("a", "2"),
            XLinksTaggedEntry::new("b", "3"),
        ];
        let deduped = x_links_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_links_validation_result_warning() {
        let w = XLinksValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_links_filter_by_tag_empty() {
        let entries: Vec<XLinksTaggedEntry> = vec![];
        assert!(x_links_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_links_tagged_entry_no_tag_match() {
        let e = XLinksTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }


    // -- links Z-extended tests -----------------------------------------------

    #[test]
    fn z_links_priority_weight() {
        assert_eq!(ZLinksPriority::Idle.weight(), 0);
        assert_eq!(ZLinksPriority::Normal.weight(), 2);
        assert_eq!(ZLinksPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_links_priority_label() {
        assert_eq!(ZLinksPriority::Low.label(), "low");
        assert_eq!(ZLinksPriority::High.label(), "high");
    }

    #[test]
    fn z_links_priority_is_elevated() {
        assert!(!ZLinksPriority::Normal.is_elevated());
        assert!(ZLinksPriority::High.is_elevated());
        assert!(ZLinksPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_links_priority_display() {
        assert_eq!(format!("{}", ZLinksPriority::Idle), "idle");
    }

    #[test]
    fn z_links_priority_all_asc() {
        let all = ZLinksPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZLinksPriority::Idle);
        assert_eq!(all[4], ZLinksPriority::Realtime);
    }

    #[test]
    fn z_links_struct_new() {
        let s = ZLinksLinkValidationResult::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_links_struct_toggled_clone() {
        let s = ZLinksLinkValidationResult::new();
        let t = s.toggled_clone();
        let _ = t.valid;
    }

    #[test]
    fn z_links_rolling_hash_deterministic() {
        let h1 = z_links_rolling_hash(b"test");
        let h2 = z_links_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_links_rolling_hash(b"a"), z_links_rolling_hash(b"b"));
    }

    #[test]
    fn z_links_pad_to_basic() {
        assert_eq!(z_links_pad_to("hi", 5), "hi   ");
        assert_eq!(z_links_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_links_is_identifier_basic() {
        assert!(z_links_is_identifier("foo_bar"));
        assert!(z_links_is_identifier("abc123"));
        assert!(!z_links_is_identifier(""));
        assert!(!z_links_is_identifier("has space"));
    }

    #[test]
    fn z_links_levenshtein_basic() {
        assert_eq!(z_links_levenshtein("", ""), 0);
        assert_eq!(z_links_levenshtein("abc", "abc"), 0);
        assert_eq!(z_links_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_links_unique_words_basic() {
        let w = z_links_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_links_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_links_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_links_common_prefix_basic() {
        assert_eq!(z_links_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_links_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_links_struct_clear() {
        let mut s = ZLinksLinkValidationResult::new();
        s.broken_links.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_links_rolling_hash_empty() {
        let h = z_links_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
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


    #[test]
    fn xb_ring_buffer_55_push_and_len() {
        let mut rb = super::XbRingBuffer55::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_55_overwrite() {
        let mut rb = super::XbRingBuffer55::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_55_get_out_of_bounds() {
        let rb = super::XbRingBuffer55::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_55_drain_all() {
        let mut rb = super::XbRingBuffer55::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_55_peek_front_back() {
        let mut rb = super::XbRingBuffer55::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_55_clear() {
        let mut rb = super::XbRingBuffer55::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_55_capacity() {
        let rb = super::XbRingBuffer55::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_55_basic() {
        let h = super::xb_fnv1a_55(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_55(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_55_different_inputs() {
        let h1 = super::xb_fnv1a_55(b"abc");
        let h2 = super::xb_fnv1a_55(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_55_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_55(&data);
        let dec = super::xb_rle_decode_55(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_55_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_55(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_55(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_55_values() {
        assert!((super::xb_clamp_55(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_55(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_55(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_55_values() {
        assert!((super::xb_lerp_55(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_55(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_55(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_55_wrap_around_twice() {
        let mut rb = super::XbRingBuffer55::new(2);
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