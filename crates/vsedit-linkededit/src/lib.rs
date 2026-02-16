//! Linked editing ranges.
//!
//! Provides types and helpers for linked editing – the ability to
//! simultaneously edit all occurrences of a symbol (e.g. matching
//! HTML open/close tags) in a document.

/// A range in a text document described by line/column coordinates (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkedEditingRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl LinkedEditingRange {
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}

/// A set of linked editing ranges, optionally constrained by a word pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedEditingRanges {
    pub ranges: Vec<LinkedEditingRange>,
    pub word_pattern: Option<String>,
}

impl LinkedEditingRanges {
    pub fn new(ranges: Vec<LinkedEditingRange>, word_pattern: Option<String>) -> Self {
        Self {
            ranges,
            word_pattern,
        }
    }
}

/// Trait for types that can provide linked editing ranges at a given position.
pub trait LinkedEditingRangeProvider {
    /// Return linked editing ranges for the document at `uri` at the given
    /// `line` and `col` (0-based), or `None` if there are no linked ranges.
    fn provide_linked_editing_ranges(
        &self,
        uri: &str,
        line: u32,
        col: u32,
    ) -> Option<LinkedEditingRanges>;
}

/// Resolve a `(line, col)` pair to a byte offset within `text`.
///
/// Lines and columns are 0-based. Returns `None` if out of bounds.
fn offset_of(text: &str, line: u32, col: u32) -> Option<usize> {
    let mut current_line = 0u32;
    let mut pos = 0usize;
    let bytes = text.as_bytes();

    // Advance to the start of the target line.
    while current_line < line {
        if pos >= bytes.len() {
            return None;
        }
        if bytes[pos] == b'\n' {
            current_line += 1;
        }
        pos += 1;
    }

    let offset = pos + col as usize;
    if offset > bytes.len() {
        None
    } else {
        Some(offset)
    }
}

/// Apply `new_text` to every range in `ranges`, replacing the original content
/// at each range. Ranges are processed from last to first so that earlier byte
/// offsets remain valid after each replacement.
///
/// Returns the edited text, or `None` if any range is out of bounds.
pub fn apply_linked_edit(
    text: &str,
    ranges: &[LinkedEditingRange],
    new_text: &str,
) -> Option<String> {
    // Convert ranges to byte offset pairs.
    let mut byte_ranges: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for r in ranges {
        let start = offset_of(text, r.start_line, r.start_col)?;
        let end = offset_of(text, r.end_line, r.end_col)?;
        if end < start {
            return None;
        }
        byte_ranges.push((start, end));
    }

    // Sort by start offset descending so replacements don't shift earlier offsets.
    byte_ranges.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = text.to_string();
    for (start, end) in byte_ranges {
        result.replace_range(start..end, new_text);
    }
    Some(result)
}

// ---------------------------------------------------------------------------
// Linked editing session
// ---------------------------------------------------------------------------

/// Configuration for linked editing behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedEditingConfig {
    /// Whether linked editing is enabled.
    pub enabled: bool,
    /// Delay in milliseconds before applying linked edits.
    pub delay_ms: u32,
}

impl Default for LinkedEditingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_ms: 0,
        }
    }
}

/// An active linked editing session tied to a specific document.
#[derive(Debug, Clone)]
pub struct LinkedEditingSession {
    /// URI of the document being edited.
    pub uri: String,
    /// The original text at the time the session started.
    pub original_text: String,
    /// The linked ranges within the document.
    pub ranges: LinkedEditingRanges,
}

impl LinkedEditingSession {
    pub fn new(uri: String, original_text: String, ranges: LinkedEditingRanges) -> Self {
        Self {
            uri,
            original_text,
            ranges,
        }
    }

    /// Apply `new_text` to every linked range, returning the resulting text
    /// or `None` if the edit cannot be applied.
    pub fn update(&mut self, new_text: &str) -> Option<String> {
        if !self.is_valid_edit(new_text) {
            return None;
        }
        apply_linked_edit(&self.original_text, &self.ranges.ranges, new_text)
    }

    /// Check whether `new_text` satisfies the session's word pattern (if any).
    pub fn is_valid_edit(&self, new_text: &str) -> bool {
        if new_text.is_empty() {
            return false;
        }
        match &self.ranges.word_pattern {
            Some(pat) => {
                // Simple check: pattern must be alphanumeric identifier-like
                if pat.is_empty() {
                    return true;
                }
                // Fall back to basic identifier validation when we don't
                // have a regex engine available.
                new_text.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            }
            None => true,
        }
    }
}

/// Check if a position (line, col) falls within a `LinkedEditingRange`.
pub fn range_contains(range: &LinkedEditingRange, line: u32, col: u32) -> bool {
    if line < range.start_line || line > range.end_line {
        return false;
    }
    if line == range.start_line && col < range.start_col {
        return false;
    }
    if line == range.end_line && col > range.end_col {
        return false;
    }
    true
}

/// Find the first range in `ranges` that contains the position `(line, col)`.
pub fn find_range_at(ranges: &[LinkedEditingRange], line: u32, col: u32) -> Option<usize> {
    ranges
        .iter()
        .position(|r| range_contains(r, line, col))
}

/// Extract the text covered by `range` from `text`.
pub fn extract_text(text: &str, range: &LinkedEditingRange) -> Option<String> {
    let start = offset_of(text, range.start_line, range.start_col)?;
    let end = offset_of(text, range.end_line, range.end_col)?;
    if end < start {
        return None;
    }
    Some(text[start..end].to_string())
}

/// Validate that all ranges are non-overlapping and in order.
pub fn validate_ranges(ranges: &[LinkedEditingRange]) -> bool {
    for window in ranges.windows(2) {
        let a = &window[0];
        let b = &window[1];
        // a must end before b starts
        if a.end_line > b.start_line {
            return false;
        }
        if a.end_line == b.start_line && a.end_col > b.start_col {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_linked_edit_single_line() {
        let text = "<div>hello</div>";
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),   // "div" in opening tag
            LinkedEditingRange::new(0, 12, 0, 15),  // "div" in closing tag
        ];
        let result = apply_linked_edit(text, &ranges, "span").unwrap();
        assert_eq!(result, "<span>hello</span>");
    }

    #[test]
    fn apply_linked_edit_multi_line() {
        let text = "<div>\n  content\n</div>";
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),   // "div" line 0
            LinkedEditingRange::new(2, 2, 2, 5),    // "div" line 2
        ];
        let result = apply_linked_edit(text, &ranges, "section").unwrap();
        assert_eq!(result, "<section>\n  content\n</section>");
    }

    #[test]
    fn apply_linked_edit_out_of_bounds() {
        let text = "short";
        let ranges = vec![LinkedEditingRange::new(5, 0, 5, 3)];
        assert!(apply_linked_edit(text, &ranges, "x").is_none());
    }

    #[test]
    fn linked_editing_range_provider_trait() {
        struct HtmlProvider;
        impl LinkedEditingRangeProvider for HtmlProvider {
            fn provide_linked_editing_ranges(
                &self,
                _uri: &str,
                _line: u32,
                _col: u32,
            ) -> Option<LinkedEditingRanges> {
                Some(LinkedEditingRanges::new(
                    vec![
                        LinkedEditingRange::new(0, 1, 0, 4),
                        LinkedEditingRange::new(0, 12, 0, 15),
                    ],
                    Some(r"[a-zA-Z][a-zA-Z0-9]*".to_string()),
                ))
            }
        }

        let provider = HtmlProvider;
        let result = provider
            .provide_linked_editing_ranges("file:///test.html", 0, 2)
            .unwrap();
        assert_eq!(result.ranges.len(), 2);
        assert_eq!(result.word_pattern.as_deref(), Some(r"[a-zA-Z][a-zA-Z0-9]*"));
    }

    #[test]
    fn linked_editing_session_update() {
        let text = "<div>hello</div>";
        let ranges = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 1, 0, 4),
                LinkedEditingRange::new(0, 12, 0, 15),
            ],
            None,
        );
        let mut session = LinkedEditingSession::new(
            "file:///a.html".into(),
            text.into(),
            ranges,
        );
        let result = session.update("span").unwrap();
        assert_eq!(result, "<span>hello</span>");
    }

    #[test]
    fn linked_editing_session_invalid_empty() {
        let text = "<div></div>";
        let ranges = LinkedEditingRanges::new(
            vec![LinkedEditingRange::new(0, 1, 0, 4)],
            None,
        );
        let mut session = LinkedEditingSession::new("f".into(), text.into(), ranges);
        assert!(session.update("").is_none());
    }

    #[test]
    fn is_valid_edit_with_word_pattern() {
        let ranges = LinkedEditingRanges::new(
            vec![LinkedEditingRange::new(0, 0, 0, 3)],
            Some(r"[a-zA-Z]+".to_string()),
        );
        let session = LinkedEditingSession::new("f".into(), "abc".into(), ranges);
        assert!(session.is_valid_edit("xyz"));
        assert!(!session.is_valid_edit("x y")); // contains space
    }

    #[test]
    fn range_contains_basic() {
        let r = LinkedEditingRange::new(1, 5, 1, 10);
        assert!(range_contains(&r, 1, 5));
        assert!(range_contains(&r, 1, 7));
        assert!(range_contains(&r, 1, 10));
        assert!(!range_contains(&r, 1, 4));
        assert!(!range_contains(&r, 1, 11));
        assert!(!range_contains(&r, 0, 7));
        assert!(!range_contains(&r, 2, 7));
    }

    #[test]
    fn find_range_at_basic() {
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),
            LinkedEditingRange::new(0, 12, 0, 15),
        ];
        assert_eq!(find_range_at(&ranges, 0, 2), Some(0));
        assert_eq!(find_range_at(&ranges, 0, 13), Some(1));
        assert_eq!(find_range_at(&ranges, 0, 6), None);
    }

    #[test]
    fn extract_text_basic() {
        let text = "<div>hello</div>";
        let r = LinkedEditingRange::new(0, 1, 0, 4);
        assert_eq!(extract_text(text, &r).unwrap(), "div");
    }

    #[test]
    fn extract_text_out_of_bounds() {
        let text = "short";
        let r = LinkedEditingRange::new(5, 0, 5, 3);
        assert!(extract_text(text, &r).is_none());
    }

    #[test]
    fn validate_ranges_valid() {
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),
            LinkedEditingRange::new(0, 12, 0, 15),
        ];
        assert!(validate_ranges(&ranges));
    }

    #[test]
    fn validate_ranges_overlapping() {
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 10),
            LinkedEditingRange::new(0, 5, 0, 15),
        ];
        assert!(!validate_ranges(&ranges));
    }

    #[test]
    fn linked_editing_config_default() {
        let cfg = LinkedEditingConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.delay_ms, 0);
    }
}
