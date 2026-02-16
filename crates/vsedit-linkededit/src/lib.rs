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
}
