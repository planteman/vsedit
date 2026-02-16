//! Linked editing ranges.
//!
//! Provides types and helpers for linked editing – the ability to
//! simultaneously edit all occurrences of a symbol (e.g. matching
//! HTML open/close tags) in a document.

use std::fmt;
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

// ---------------------------------------------------------------------------
// Additional methods on LinkedEditingRange
// ---------------------------------------------------------------------------

impl LinkedEditingRange {
    /// Character length of the range when it spans a single line.
    /// Returns `None` for multi-line ranges.
    pub fn len(&self) -> Option<u32> {
        if self.start_line == self.end_line {
            Some(self.end_col - self.start_col)
        } else {
            None
        }
    }

    /// Returns `true` when both start and end are on the same line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    /// Returns `true` when the given `(line, col)` position is inside this range.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        range_contains(self, line, col)
    }

    /// Returns `true` when this range overlaps with `other`.
    pub fn overlaps(&self, other: &LinkedEditingRange) -> bool {
        // No overlap if one ends before the other starts.
        if self.end_line < other.start_line {
            return false;
        }
        if other.end_line < self.start_line {
            return false;
        }
        if self.end_line == other.start_line && self.end_col <= other.start_col {
            return false;
        }
        if other.end_line == self.start_line && other.end_col <= self.start_col {
            return false;
        }
        true
    }
}

impl PartialOrd for LinkedEditingRange {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LinkedEditingRange {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.start_line, self.start_col, self.end_line, self.end_col)
            .cmp(&(other.start_line, other.start_col, other.end_line, other.end_col))
    }
}

impl std::fmt::Display for LinkedEditingRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

// ---------------------------------------------------------------------------
// Additional methods on LinkedEditingRanges
// ---------------------------------------------------------------------------

impl LinkedEditingRanges {
    /// Returns `true` when all ranges are non-overlapping and ordered.
    pub fn is_valid(&self) -> bool {
        validate_ranges(&self.ranges)
    }

    /// Number of ranges in this set.
    pub fn range_count(&self) -> usize {
        self.ranges.len()
    }

    /// Find the index of the first range that contains the given position.
    pub fn find_at_position(&self, line: u32, col: u32) -> Option<usize> {
        find_range_at(&self.ranges, line, col)
    }
}

// ---------------------------------------------------------------------------
// Additional methods on LinkedEditingSession
// ---------------------------------------------------------------------------

impl LinkedEditingSession {
    /// Number of linked ranges in the session.
    pub fn range_count(&self) -> usize {
        self.ranges.range_count()
    }

    /// The optional word pattern constraining edits, if any.
    pub fn word_pattern(&self) -> Option<&str> {
        self.ranges.word_pattern.as_deref()
    }

    /// Extract the current text at the given range index.
    pub fn text_at_range(&self, index: usize) -> Option<String> {
        let r = self.ranges.ranges.get(index)?;
        extract_text(&self.original_text, r)
    }
}

// ---------------------------------------------------------------------------
// Range utilities
// ---------------------------------------------------------------------------

/// Sort a slice of ranges by start position (line, col).
pub fn sort_ranges(ranges: &mut [LinkedEditingRange]) {
    ranges.sort();
}

/// Merge adjacent or overlapping ranges into a minimal set.
/// The input does **not** need to be sorted; this function sorts first.
pub fn merge_ranges(ranges: &[LinkedEditingRange]) -> Vec<LinkedEditingRange> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<LinkedEditingRange> = ranges.to_vec();
    sort_ranges(&mut sorted);

    let mut merged: Vec<LinkedEditingRange> = vec![sorted[0]];
    for r in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        // Check if `r` starts before or at the end of `last`.
        let adjacent_or_overlapping = (r.start_line < last.end_line)
            || (r.start_line == last.end_line && r.start_col <= last.end_col);
        if adjacent_or_overlapping {
            // Extend `last` to cover `r` as well.
            if r.end_line > last.end_line
                || (r.end_line == last.end_line && r.end_col > last.end_col)
            {
                last.end_line = r.end_line;
                last.end_col = r.end_col;
            }
        } else {
            merged.push(*r);
        }
    }
    merged
}

/// Describes how much subsequent text is shifted after replacing a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeShift {
    /// Index of the range that was replaced.
    pub range_index: usize,
    /// Byte delta applied at the replacement site (positive = text grew).
    pub byte_delta: i64,
}

/// Compute the byte shifts that result from replacing every range in `ranges`
/// with `new_text` within `text`.
///
/// Returns one `RangeShift` per range, in the order they appear.
/// Returns `None` if any range is out of bounds.
pub fn compute_shifts(
    text: &str,
    ranges: &[LinkedEditingRange],
    new_text: &str,
) -> Option<Vec<RangeShift>> {
    let new_len = new_text.len() as i64;
    let mut shifts = Vec::with_capacity(ranges.len());
    for (i, r) in ranges.iter().enumerate() {
        let start = offset_of(text, r.start_line, r.start_col)?;
        let end = offset_of(text, r.end_line, r.end_col)?;
        if end < start {
            return None;
        }
        let old_len = (end - start) as i64;
        shifts.push(RangeShift {
            range_index: i,
            byte_delta: new_len - old_len,
        });
    }
    Some(shifts)
}

/// Simple history stack for linked editing undo support.
#[derive(Debug, Clone)]
pub struct LinkedEditingHistory {
    entries: Vec<String>,
    capacity: usize,
}

// ---------------------------------------------------------------------------
// LinkedEditGroup
// ---------------------------------------------------------------------------

/// A group of edit ranges that should be edited together.
///
/// Unlike `LinkedEditingRanges` which works with line/col positions,
/// `LinkedEditGroup` works with byte offsets for simpler text manipulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedEditGroup {
    /// Byte offset ranges as `(start, end)` pairs.
    pub ranges: Vec<(usize, usize)>,
    /// The current text at each range (should all be identical).
    pub current_text: String,
}

impl LinkedEditGroup {
    /// Create a new edit group from byte-offset ranges and the current text.
    pub fn new(ranges: Vec<(usize, usize)>, current_text: impl Into<String>) -> Self {
        Self {
            ranges,
            current_text: current_text.into(),
        }
    }

    /// Number of linked ranges in this group.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Returns `true` if the group has no ranges.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}

impl fmt::Display for LinkedEditGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LinkedEditGroup({} ranges, text='{}')",
            self.ranges.len(),
            self.current_text
        )
    }
}

/// Apply a text change to all ranges in a `LinkedEditGroup`.
///
/// Replaces the text at each range with `new_text`, processing from last to
/// first so that byte offsets remain valid. Returns the edited text or `None`
/// if any range is out of bounds.
pub fn apply_group_edit(text: &str, group: &LinkedEditGroup, new_text: &str) -> Option<String> {
    let mut sorted_ranges = group.ranges.clone();
    sorted_ranges.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = text.to_string();
    for (start, end) in sorted_ranges {
        if start > result.len() || end > result.len() || end < start {
            return None;
        }
        result.replace_range(start..end, new_text);
    }
    Some(result)
}

/// Validate that the ranges in a `LinkedEditGroup` do not overlap.
pub fn validate_linked_ranges(group: &LinkedEditGroup) -> bool {
    let mut sorted = group.ranges.clone();
    sorted.sort_by_key(|r| r.0);
    for window in sorted.windows(2) {
        if window[0].1 > window[1].0 {
            return false;
        }
    }
    true
}

impl LinkedEditingHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Push a snapshot onto the history. If the history exceeds capacity the
    /// oldest entry is discarded.
    pub fn push(&mut self, snapshot: String) {
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(snapshot);
    }

    /// Pop the most recent snapshot (undo).
    pub fn pop(&mut self) -> Option<String> {
        self.entries.pop()
    }

    /// Number of entries currently in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Word boundary detection for linked editing
// ---------------------------------------------------------------------------

/// Detect word boundaries in the given text, returning ranges `(start_byte, end_byte)`
/// for each word. A "word" is a contiguous run of alphanumeric or underscore characters.
pub fn linked_edit_detect_word_boundaries(text: &str) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if is_word_byte(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_word_byte(bytes[i]) {
                i += 1;
            }
            result.push((start, i));
        } else {
            i += 1;
        }
    }
    result
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Find all occurrences of `word` in `text` and return their byte offset ranges.
pub fn linked_edit_find_word_occurrences(text: &str, word: &str) -> Vec<(usize, usize)> {
    if word.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    let mut start = 0;
    while let Some(pos) = text[start..].find(word) {
        let abs_pos = start + pos;
        let end_pos = abs_pos + word.len();
        // Ensure it's a whole-word match
        let before_ok = abs_pos == 0 || !is_word_byte(text.as_bytes()[abs_pos - 1]);
        let after_ok = end_pos >= text.len() || !is_word_byte(text.as_bytes()[end_pos]);
        if before_ok && after_ok {
            results.push((abs_pos, end_pos));
        }
        start = abs_pos + 1;
    }
    results
}

/// Build a `LinkedEditGroup` from all occurrences of `word` in `text`.
pub fn linked_edit_group_from_word(text: &str, word: &str) -> LinkedEditGroup {
    let ranges = linked_edit_find_word_occurrences(text, word);
    LinkedEditGroup::new(ranges, word)
}

/// Convert a byte-offset range to a `LinkedEditingRange` using (line, col) coordinates.
pub fn byte_range_to_editing_range(text: &str, start: usize, end: usize) -> Option<LinkedEditingRange> {
    let (start_line, start_col) = byte_offset_to_line_col(text, start)?;
    let (end_line, end_col) = byte_offset_to_line_col(text, end)?;
    Some(LinkedEditingRange::new(start_line, start_col, end_line, end_col))
}

fn byte_offset_to_line_col(text: &str, offset: usize) -> Option<(u32, u32)> {
    if offset > text.len() {
        return None;
    }
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, byte) in text.bytes().enumerate() {
        if i == offset {
            return Some((line, col));
        }
        if byte == b'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    if offset == text.len() {
        Some((line, col))
    } else {
        None
    }
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

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn range_len_single_line() {
        let r = LinkedEditingRange::new(0, 2, 0, 7);
        assert_eq!(r.len(), Some(5));
    }

    #[test]
    fn range_len_multi_line_returns_none() {
        let r = LinkedEditingRange::new(0, 2, 1, 7);
        assert_eq!(r.len(), None);
    }

    #[test]
    fn range_is_single_line() {
        assert!(LinkedEditingRange::new(3, 0, 3, 10).is_single_line());
        assert!(!LinkedEditingRange::new(3, 0, 4, 10).is_single_line());
    }

    #[test]
    fn range_contains_method() {
        let r = LinkedEditingRange::new(1, 5, 1, 10);
        assert!(r.contains(1, 5));
        assert!(r.contains(1, 7));
        assert!(!r.contains(1, 11));
    }

    #[test]
    fn range_overlaps() {
        let a = LinkedEditingRange::new(0, 0, 0, 5);
        let b = LinkedEditingRange::new(0, 3, 0, 8);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn range_no_overlap() {
        let a = LinkedEditingRange::new(0, 0, 0, 5);
        let b = LinkedEditingRange::new(0, 5, 0, 10);
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn range_overlaps_multi_line() {
        let a = LinkedEditingRange::new(0, 0, 1, 5);
        let b = LinkedEditingRange::new(1, 3, 2, 0);
        assert!(a.overlaps(&b));
    }

    #[test]
    fn ranges_is_valid_method() {
        let valid = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 0, 0, 3),
                LinkedEditingRange::new(0, 5, 0, 8),
            ],
            None,
        );
        assert!(valid.is_valid());

        let invalid = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 0, 0, 6),
                LinkedEditingRange::new(0, 5, 0, 8),
            ],
            None,
        );
        assert!(!invalid.is_valid());
    }

    #[test]
    fn ranges_range_count() {
        let r = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 0, 0, 3),
                LinkedEditingRange::new(0, 5, 0, 8),
            ],
            None,
        );
        assert_eq!(r.range_count(), 2);
    }

    #[test]
    fn ranges_find_at_position_method() {
        let r = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 0, 0, 3),
                LinkedEditingRange::new(0, 5, 0, 8),
            ],
            None,
        );
        assert_eq!(r.find_at_position(0, 1), Some(0));
        assert_eq!(r.find_at_position(0, 6), Some(1));
        assert_eq!(r.find_at_position(0, 4), None);
    }

    #[test]
    fn session_range_count() {
        let session = LinkedEditingSession::new(
            "file:///a.html".into(),
            "<div></div>".into(),
            LinkedEditingRanges::new(
                vec![
                    LinkedEditingRange::new(0, 1, 0, 4),
                    LinkedEditingRange::new(0, 7, 0, 10),
                ],
                None,
            ),
        );
        assert_eq!(session.range_count(), 2);
    }

    #[test]
    fn session_word_pattern() {
        let session = LinkedEditingSession::new(
            "f".into(),
            "abc".into(),
            LinkedEditingRanges::new(
                vec![LinkedEditingRange::new(0, 0, 0, 3)],
                Some("ident".into()),
            ),
        );
        assert_eq!(session.word_pattern(), Some("ident"));
    }

    #[test]
    fn session_text_at_range() {
        let session = LinkedEditingSession::new(
            "f".into(),
            "<div>hello</div>".into(),
            LinkedEditingRanges::new(
                vec![
                    LinkedEditingRange::new(0, 1, 0, 4),
                    LinkedEditingRange::new(0, 12, 0, 15),
                ],
                None,
            ),
        );
        assert_eq!(session.text_at_range(0), Some("div".into()));
        assert_eq!(session.text_at_range(1), Some("div".into()));
        assert_eq!(session.text_at_range(2), None);
    }

    #[test]
    fn sort_ranges_works() {
        let mut ranges = vec![
            LinkedEditingRange::new(2, 0, 2, 3),
            LinkedEditingRange::new(0, 0, 0, 3),
            LinkedEditingRange::new(1, 5, 1, 8),
        ];
        sort_ranges(&mut ranges);
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[1].start_line, 1);
        assert_eq!(ranges[2].start_line, 2);
    }

    #[test]
    fn merge_ranges_overlapping() {
        let ranges = vec![
            LinkedEditingRange::new(0, 0, 0, 5),
            LinkedEditingRange::new(0, 3, 0, 8),
        ];
        let merged = merge_ranges(&ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], LinkedEditingRange::new(0, 0, 0, 8));
    }

    #[test]
    fn merge_ranges_adjacent() {
        let ranges = vec![
            LinkedEditingRange::new(0, 0, 0, 5),
            LinkedEditingRange::new(0, 5, 0, 10),
        ];
        let merged = merge_ranges(&ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], LinkedEditingRange::new(0, 0, 0, 10));
    }

    #[test]
    fn merge_ranges_disjoint() {
        let ranges = vec![
            LinkedEditingRange::new(0, 0, 0, 3),
            LinkedEditingRange::new(0, 6, 0, 9),
        ];
        let merged = merge_ranges(&ranges);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_ranges_empty() {
        let merged = merge_ranges(&[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn compute_shifts_basic() {
        let text = "<div>hello</div>";
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),
            LinkedEditingRange::new(0, 12, 0, 15),
        ];
        let shifts = compute_shifts(text, &ranges, "span").unwrap();
        assert_eq!(shifts.len(), 2);
        // "div" (3 bytes) -> "span" (4 bytes) => delta = +1
        assert_eq!(shifts[0].byte_delta, 1);
        assert_eq!(shifts[1].byte_delta, 1);
    }

    #[test]
    fn compute_shifts_shrink() {
        let text = "<section></section>";
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 8),
            LinkedEditingRange::new(0, 11, 0, 18),
        ];
        let shifts = compute_shifts(text, &ranges, "p").unwrap();
        // "section" (7) -> "p" (1) => delta = -6
        assert_eq!(shifts[0].byte_delta, -6);
    }

    #[test]
    fn compute_shifts_out_of_bounds() {
        let text = "hi";
        let ranges = vec![LinkedEditingRange::new(5, 0, 5, 3)];
        assert!(compute_shifts(text, &ranges, "x").is_none());
    }

    #[test]
    fn history_push_pop() {
        let mut h = LinkedEditingHistory::new(3);
        assert!(h.is_empty());
        h.push("a".into());
        h.push("b".into());
        assert_eq!(h.len(), 2);
        assert_eq!(h.pop(), Some("b".into()));
        assert_eq!(h.pop(), Some("a".into()));
        assert!(h.is_empty());
    }

    #[test]
    fn history_capacity() {
        let mut h = LinkedEditingHistory::new(2);
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        assert_eq!(h.len(), 2);
        // oldest ("a") was dropped
        assert_eq!(h.pop(), Some("c".into()));
        assert_eq!(h.pop(), Some("b".into()));
    }

    #[test]
    fn display_linked_editing_range() {
        let r = LinkedEditingRange::new(1, 5, 3, 10);
        assert_eq!(format!("{}", r), "1:5-3:10");
    }

    #[test]
    fn ord_linked_editing_range() {
        let a = LinkedEditingRange::new(0, 0, 0, 5);
        let b = LinkedEditingRange::new(0, 3, 0, 8);
        let c = LinkedEditingRange::new(1, 0, 1, 2);
        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn range_single_char() {
        let r = LinkedEditingRange::new(0, 5, 0, 6);
        assert_eq!(r.len(), Some(1));
        assert!(r.is_single_line());
        assert!(r.contains(0, 5));
        assert!(r.contains(0, 6));
        assert!(!r.contains(0, 7));
    }

    #[test]
    fn range_empty() {
        let r = LinkedEditingRange::new(0, 5, 0, 5);
        assert_eq!(r.len(), Some(0));
        assert!(r.is_single_line());
    }

    // ── LinkedEditGroup tests ──

    #[test]
    fn edit_group_new() {
        let group = LinkedEditGroup::new(vec![(1, 4), (12, 15)], "div");
        assert_eq!(group.len(), 2);
        assert!(!group.is_empty());
        assert_eq!(group.current_text, "div");
    }

    #[test]
    fn edit_group_empty() {
        let group = LinkedEditGroup::new(vec![], "");
        assert!(group.is_empty());
    }

    #[test]
    fn edit_group_display() {
        let group = LinkedEditGroup::new(vec![(0, 3)], "abc");
        let s = format!("{group}");
        assert!(s.contains("1 ranges"));
        assert!(s.contains("abc"));
    }

    #[test]
    fn apply_group_edit_basic() {
        let text = "<div>hello</div>";
        let group = LinkedEditGroup::new(vec![(1, 4), (12, 15)], "div");
        let result = apply_group_edit(text, &group, "span").unwrap();
        assert_eq!(result, "<span>hello</span>");
    }

    #[test]
    fn apply_group_edit_out_of_bounds() {
        let text = "short";
        let group = LinkedEditGroup::new(vec![(0, 100)], "short");
        assert!(apply_group_edit(text, &group, "x").is_none());
    }

    #[test]
    fn validate_linked_ranges_valid() {
        let group = LinkedEditGroup::new(vec![(0, 3), (5, 8), (10, 13)], "abc");
        assert!(validate_linked_ranges(&group));
    }

    #[test]
    fn validate_linked_ranges_overlapping() {
        let group = LinkedEditGroup::new(vec![(0, 5), (3, 8)], "abc");
        assert!(!validate_linked_ranges(&group));
    }

    #[test]
    fn validate_linked_ranges_empty() {
        let group = LinkedEditGroup::new(vec![], "");
        assert!(validate_linked_ranges(&group));
    }

    #[test]
    fn apply_group_edit_single_range() {
        let text = "hello world";
        let group = LinkedEditGroup::new(vec![(6, 11)], "world");
        let result = apply_group_edit(text, &group, "rust").unwrap();
        assert_eq!(result, "hello rust");
    }

    #[test]
    fn detect_word_boundaries_basic() {
        let boundaries = linked_edit_detect_word_boundaries("hello world foo_bar");
        assert_eq!(boundaries, vec![(0, 5), (6, 11), (12, 19)]);
    }

    #[test]
    fn detect_word_boundaries_empty() {
        let boundaries = linked_edit_detect_word_boundaries("");
        assert!(boundaries.is_empty());
    }

    #[test]
    fn detect_word_boundaries_punctuation() {
        let boundaries = linked_edit_detect_word_boundaries("a.b(c)");
        assert_eq!(boundaries, vec![(0, 1), (2, 3), (4, 5)]);
    }

    #[test]
    fn find_word_occurrences_basic() {
        let text = "let x = x + x;";
        let occs = linked_edit_find_word_occurrences(text, "x");
        assert_eq!(occs.len(), 3);
    }

    #[test]
    fn find_word_occurrences_no_partial() {
        let text = "fox foxes";
        let occs = linked_edit_find_word_occurrences(text, "fox");
        assert_eq!(occs.len(), 1); // "foxes" should not match
        assert_eq!(occs[0], (0, 3));
    }

    #[test]
    fn linked_edit_group_from_word_basic() {
        let text = "fn foo() { foo(); }";
        let group = linked_edit_group_from_word(text, "foo");
        assert_eq!(group.ranges.len(), 2);
        assert_eq!(group.current_text, "foo");
    }

    #[test]
    fn byte_range_to_editing_range_single_line() {
        let text = "hello world";
        let range = byte_range_to_editing_range(text, 6, 11).unwrap();
        assert_eq!(range.start_line, 0);
        assert_eq!(range.start_col, 6);
        assert_eq!(range.end_col, 11);
    }

    #[test]
    fn byte_range_to_editing_range_multiline() {
        let text = "line1\nline2\nline3";
        let range = byte_range_to_editing_range(text, 6, 11).unwrap();
        assert_eq!(range.start_line, 1);
        assert_eq!(range.start_col, 0);
        assert_eq!(range.end_line, 1);
        assert_eq!(range.end_col, 5);
    }
}
