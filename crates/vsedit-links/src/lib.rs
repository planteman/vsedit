//! Document link detection for editor content.
//!
//! Provides URL and file-path detection within text, as well as a trait for
//! language-specific document-link providers.

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
// Tests
// ---------------------------------------------------------------------------

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
}
