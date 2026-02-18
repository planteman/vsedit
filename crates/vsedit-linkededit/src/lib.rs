//! Linked editing ranges.
//!
//! Provides types and helpers for linked editing – the ability to
//! simultaneously edit all occurrences of a symbol (e.g. matching
//! HTML open/close tags) in a document.

use std::collections::HashMap;
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
    /// Number of lines this range spans (inclusive).
    pub fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

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
            "Ln {}:Col {} - Ln {}:Col {}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

// ---------------------------------------------------------------------------
// Additional methods on LinkedEditingRanges
// ---------------------------------------------------------------------------

impl LinkedEditingRanges {
    /// Returns `true` when the set contains no ranges.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Returns `true` when a word pattern constraint is set.
    pub fn has_word_pattern(&self) -> bool {
        self.word_pattern.is_some()
    }

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

impl fmt::Display for LinkedEditingRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} linked ranges", self.ranges.len())
    }
}

// ---------------------------------------------------------------------------
// Additional methods on LinkedEditingSession
// ---------------------------------------------------------------------------

impl LinkedEditingSession {
    /// URI of the document being edited.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// The original text at the time the session started.
    pub fn original_text(&self) -> &str {
        &self.original_text
    }

    /// Number of edits that have been applied (counted via `update` calls that
    /// mutated `original_text`). We track this by comparing `original_text`
    /// snapshots; for now expose the range count as a proxy for how many
    /// simultaneous edits each `update` performs.
    pub fn edit_count(&self) -> usize {
        self.ranges.range_count()
    }

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

// ---------------------------------------------------------------------------
// LinkedEditingUndoStack – undo/redo for linked edits
// ---------------------------------------------------------------------------

/// Undo/redo stack for linked editing operations.
#[derive(Debug, Clone)]
pub struct LinkedEditingUndoStack {
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    capacity: usize,
}

impl LinkedEditingUndoStack {
    pub fn new(capacity: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            capacity,
        }
    }

    /// Push a new state snapshot. Clears the redo stack.
    pub fn push(&mut self, snapshot: String) {
        if self.undo_stack.len() >= self.capacity {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();
    }

    /// Undo: pop from undo stack and push current state to redo.
    /// Returns the previous state, or `None` if nothing to undo.
    pub fn undo(&mut self, current_state: &str) -> Option<String> {
        let prev = self.undo_stack.pop()?;
        self.redo_stack.push(current_state.to_string());
        Some(prev)
    }

    /// Redo: pop from redo stack and push current state to undo.
    /// Returns the next state, or `None` if nothing to redo.
    pub fn redo(&mut self, current_state: &str) -> Option<String> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current_state.to_string());
        Some(next)
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of undo entries.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of redo entries.
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
}

// ---------------------------------------------------------------------------
// RangeHighlighter – highlight active linked ranges
// ---------------------------------------------------------------------------

/// Style for highlighting a range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HighlightStyle {
    Primary,
    Secondary,
    Inactive,
}

/// A highlighted range with style information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedRange {
    pub range: LinkedEditingRange,
    pub style: HighlightStyle,
}

/// Produces highlight decorations for linked editing ranges.
pub struct RangeHighlighter;

impl RangeHighlighter {
    /// Highlight all ranges: the one containing the cursor gets `Primary`,
    /// all others get `Secondary`.
    pub fn highlight(
        ranges: &[LinkedEditingRange],
        cursor_line: u32,
        cursor_col: u32,
    ) -> Vec<HighlightedRange> {
        let active_idx = find_range_at(ranges, cursor_line, cursor_col);
        ranges
            .iter()
            .enumerate()
            .map(|(i, r)| HighlightedRange {
                range: *r,
                style: if Some(i) == active_idx {
                    HighlightStyle::Primary
                } else {
                    HighlightStyle::Secondary
                },
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// LinkedEditValidator – validate edit operations before applying
// ---------------------------------------------------------------------------

/// Validation error for linked edit operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedEditValidationError {
    EmptyNewText,
    OverlappingRanges,
    OutOfBounds,
    PatternMismatch,
}

/// Validates linked edit operations before they are applied.
pub struct LinkedEditValidator;

impl LinkedEditValidator {
    /// Validate that a linked edit can be applied.
    pub fn validate(
        text: &str,
        ranges: &[LinkedEditingRange],
        new_text: &str,
        word_pattern: Option<&str>,
    ) -> Result<(), Vec<LinkedEditValidationError>> {
        let mut errors = Vec::new();

        if new_text.is_empty() {
            errors.push(LinkedEditValidationError::EmptyNewText);
        }

        if !validate_ranges(ranges) {
            errors.push(LinkedEditValidationError::OverlappingRanges);
        }

        // Check all ranges are within text bounds.
        for r in ranges {
            if offset_of(text, r.start_line, r.start_col).is_none()
                || offset_of(text, r.end_line, r.end_col).is_none()
            {
                errors.push(LinkedEditValidationError::OutOfBounds);
                break;
            }
        }

        // Pattern check.
        if let Some(pat) = word_pattern {
            if !pat.is_empty()
                && !new_text.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
                errors.push(LinkedEditValidationError::PatternMismatch);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ---------------------------------------------------------------------------
// Extensions on LinkedEditGroup
// ---------------------------------------------------------------------------

impl LinkedEditGroup {
    /// Create a deep copy of this group with an optional new current_text.
    pub fn clone_group(&self, new_text: Option<&str>) -> LinkedEditGroup {
        LinkedEditGroup {
            ranges: self.ranges.clone(),
            current_text: new_text.unwrap_or(&self.current_text).to_string(),
        }
    }

    /// Split this group into two groups at the given range index.
    /// The first group contains ranges `[0, at)`, the second `[at, len)`.
    pub fn split_at(&self, at: usize) -> (LinkedEditGroup, LinkedEditGroup) {
        let (left, right) = self.ranges.split_at(at.min(self.ranges.len()));
        (
            LinkedEditGroup::new(left.to_vec(), &self.current_text),
            LinkedEditGroup::new(right.to_vec(), &self.current_text),
        )
    }
}

/// Compute the total character span across all ranges, assuming single-line ranges.
/// Returns `None` if any range is multi-line.
pub fn total_span(ranges: &[LinkedEditingRange]) -> Option<u32> {
    let mut total = 0u32;
    for r in ranges {
        if r.start_line != r.end_line {
            return None;
        }
        total += r.end_col.saturating_sub(r.start_col);
    }
    Some(total)
}

/// Return only the ranges that are on a given line.
pub fn ranges_on_line(ranges: &[LinkedEditingRange], line: u32) -> Vec<LinkedEditingRange> {
    ranges
        .iter()
        .filter(|r| r.start_line <= line && r.end_line >= line)
        .copied()
        .collect()
}

/// Shift all ranges by a line delta (positive = down, wraps at zero for negative).
pub fn shift_ranges(ranges: &[LinkedEditingRange], line_delta: i32) -> Vec<LinkedEditingRange> {
    ranges
        .iter()
        .map(|r| LinkedEditingRange {
            start_line: (r.start_line as i64 + line_delta as i64).max(0) as u32,
            start_col: r.start_col,
            end_line: (r.end_line as i64 + line_delta as i64).max(0) as u32,
            end_col: r.end_col,
        })
        .collect()
}

/// Check whether two slices of ranges are identical in position.
pub fn ranges_equal(a: &[LinkedEditingRange], b: &[LinkedEditingRange]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

/// Deduplicate ranges, removing exact duplicates while preserving order.
pub fn deduplicate_ranges(ranges: &[LinkedEditingRange]) -> Vec<LinkedEditingRange> {
    let mut seen = Vec::new();
    for r in ranges {
        if !seen.contains(r) {
            seen.push(*r);
        }
    }
    seen
}

/// Return the smallest bounding range that contains all given ranges.
pub fn bounding_range(ranges: &[LinkedEditingRange]) -> Option<LinkedEditingRange> {
    if ranges.is_empty() {
        return None;
    }
    let mut min_line = u32::MAX;
    let mut min_col = u32::MAX;
    let mut max_line = 0u32;
    let mut max_col = 0u32;
    for r in ranges {
        if r.start_line < min_line || (r.start_line == min_line && r.start_col < min_col) {
            min_line = r.start_line;
            min_col = r.start_col;
        }
        if r.end_line > max_line || (r.end_line == max_line && r.end_col > max_col) {
            max_line = r.end_line;
            max_col = r.end_col;
        }
    }
    Some(LinkedEditingRange::new(min_line, min_col, max_line, max_col))
}

// ---------------------------------------------------------------------------
// Edit propagation – compute new ranges after an edit is applied
// ---------------------------------------------------------------------------

/// After replacing every range with `new_text`, compute the updated range
/// positions in the resulting document. This is essential for keeping the
/// linked editing session alive after an edit: the ranges must be adjusted to
/// reflect the new text lengths.
///
/// Returns `None` if any range is out of bounds in the original `text`.
pub fn propagate_ranges(
    text: &str,
    ranges: &[LinkedEditingRange],
    new_text: &str,
) -> Option<Vec<LinkedEditingRange>> {
    if ranges.is_empty() {
        return Some(Vec::new());
    }

    // Convert to byte offsets and sort by start ascending.
    let mut indexed: Vec<(usize, usize, usize)> = Vec::with_capacity(ranges.len());
    for (i, r) in ranges.iter().enumerate() {
        let start = offset_of(text, r.start_line, r.start_col)?;
        let end = offset_of(text, r.end_line, r.end_col)?;
        if end < start {
            return None;
        }
        indexed.push((i, start, end));
    }
    indexed.sort_by_key(|&(_, s, _)| s);

    let new_len = new_text.len();
    let mut cumulative_delta: i64 = 0;
    // We'll build the result text to derive line/col from byte offsets.
    let result_text = apply_linked_edit(text, ranges, new_text)?;

    let mut new_ranges = vec![LinkedEditingRange::new(0, 0, 0, 0); ranges.len()];
    for &(orig_idx, start, end) in &indexed {
        let old_len = end - start;
        let new_start = (start as i64 + cumulative_delta) as usize;
        let new_end = new_start + new_len;
        let (sl, sc) = byte_offset_to_line_col(&result_text, new_start)?;
        let (el, ec) = byte_offset_to_line_col(&result_text, new_end)?;
        new_ranges[orig_idx] = LinkedEditingRange::new(sl, sc, el, ec);
        cumulative_delta += new_len as i64 - old_len as i64;
    }
    Some(new_ranges)
}

// ---------------------------------------------------------------------------
// Conflict detection – detect conflicting edits across multiple groups
// ---------------------------------------------------------------------------

/// Represents a conflict between two edit groups that share overlapping ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditConflict {
    /// Index of the first group.
    pub group_a: usize,
    /// Index of the second group.
    pub group_b: usize,
    /// Indices of overlapping range pairs: (range in A, range in B).
    pub overlapping_pairs: Vec<(usize, usize)>,
}

/// Detect conflicts between multiple `LinkedEditGroup`s. Two groups conflict
/// when any of their byte-offset ranges overlap.
pub fn detect_conflicts(groups: &[LinkedEditGroup]) -> Vec<EditConflict> {
    let mut conflicts = Vec::new();
    for (i, ga) in groups.iter().enumerate() {
        for (j, gb) in groups.iter().enumerate().skip(i + 1) {
            let mut pairs = Vec::new();
            for (ai, &(a_start, a_end)) in ga.ranges.iter().enumerate() {
                for (bi, &(b_start, b_end)) in gb.ranges.iter().enumerate() {
                    if a_start < b_end && b_start < a_end {
                        pairs.push((ai, bi));
                    }
                }
            }
            if !pairs.is_empty() {
                conflicts.push(EditConflict {
                    group_a: i,
                    group_b: j,
                    overlapping_pairs: pairs,
                });
            }
        }
    }
    conflicts
}

// ---------------------------------------------------------------------------
// Cursor tracking across linked ranges
// ---------------------------------------------------------------------------

/// Describes a cursor position relative to a linked editing range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorMapping {
    /// Index of the source range that contains the cursor.
    pub source_range: usize,
    /// Offset of the cursor within the source range (in columns, single-line only).
    pub offset_in_range: u32,
}

/// For a cursor at `(line, col)`, compute the equivalent cursor positions in
/// all other linked ranges. Returns `None` if the cursor is not inside any
/// range, or if any range is multi-line.
pub fn map_cursor_to_linked_ranges(
    ranges: &[LinkedEditingRange],
    cursor_line: u32,
    cursor_col: u32,
) -> Option<Vec<(u32, u32)>> {
    let source_idx = find_range_at(ranges, cursor_line, cursor_col)?;
    let source = &ranges[source_idx];
    if !source.is_single_line() {
        return None;
    }
    let offset = cursor_col - source.start_col;

    let mut positions = Vec::with_capacity(ranges.len());
    for r in ranges {
        if !r.is_single_line() {
            return None;
        }
        positions.push((r.start_line, r.start_col + offset));
    }
    Some(positions)
}

/// Like `map_cursor_to_linked_ranges` but also returns the `CursorMapping`
/// metadata for the source range.
pub fn map_cursor_with_metadata(
    ranges: &[LinkedEditingRange],
    cursor_line: u32,
    cursor_col: u32,
) -> Option<(CursorMapping, Vec<(u32, u32)>)> {
    let source_idx = find_range_at(ranges, cursor_line, cursor_col)?;
    let source = &ranges[source_idx];
    if !source.is_single_line() {
        return None;
    }
    let offset = cursor_col - source.start_col;
    let mapping = CursorMapping {
        source_range: source_idx,
        offset_in_range: offset,
    };

    let positions = map_cursor_to_linked_ranges(ranges, cursor_line, cursor_col)?;
    Some((mapping, positions))
}

// ---------------------------------------------------------------------------
// Range intersection and subtraction
// ---------------------------------------------------------------------------

/// Compute the intersection of two ranges, or `None` if they don't overlap.
pub fn range_intersection(
    a: &LinkedEditingRange,
    b: &LinkedEditingRange,
) -> Option<LinkedEditingRange> {
    if !a.overlaps(b) {
        return None;
    }
    let start_line;
    let start_col;
    if a.start_line > b.start_line || (a.start_line == b.start_line && a.start_col > b.start_col) {
        start_line = a.start_line;
        start_col = a.start_col;
    } else {
        start_line = b.start_line;
        start_col = b.start_col;
    }
    let end_line;
    let end_col;
    if a.end_line < b.end_line || (a.end_line == b.end_line && a.end_col < b.end_col) {
        end_line = a.end_line;
        end_col = a.end_col;
    } else {
        end_line = b.end_line;
        end_col = b.end_col;
    }
    Some(LinkedEditingRange::new(start_line, start_col, end_line, end_col))
}

/// Filter a set of ranges to only those that contain the given position.
pub fn ranges_containing_position(
    ranges: &[LinkedEditingRange],
    line: u32,
    col: u32,
) -> Vec<(usize, LinkedEditingRange)> {
    ranges
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contains(line, col))
        .map(|(i, r)| (i, *r))
        .collect()
}

// ---------------------------------------------------------------------------
// Multi-document linked editing coordinator
// ---------------------------------------------------------------------------

/// Tracks linked editing sessions across multiple documents.
#[derive(Debug, Clone, Default)]
pub struct LinkedEditingCoordinator {
    sessions: Vec<LinkedEditingSession>,
}

impl LinkedEditingCoordinator {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    /// Start a new linked editing session for the given URI. Replaces any
    /// existing session for the same URI.
    pub fn start_session(
        &mut self,
        uri: String,
        text: String,
        ranges: LinkedEditingRanges,
    ) -> usize {
        // Remove existing session for this URI if any.
        self.sessions.retain(|s| s.uri != uri);
        let idx = self.sessions.len();
        self.sessions
            .push(LinkedEditingSession::new(uri, text, ranges));
        idx
    }

    /// End the session for the given URI. Returns `true` if a session was removed.
    pub fn end_session(&mut self, uri: &str) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.uri != uri);
        self.sessions.len() < before
    }

    /// Find the session for the given URI.
    pub fn session_for(&self, uri: &str) -> Option<&LinkedEditingSession> {
        self.sessions.iter().find(|s| s.uri == uri)
    }

    /// Find the session for the given URI (mutable).
    pub fn session_for_mut(&mut self, uri: &str) -> Option<&mut LinkedEditingSession> {
        self.sessions.iter_mut().find(|s| s.uri == uri)
    }

    /// Number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// All URIs with active sessions.
    pub fn active_uris(&self) -> Vec<&str> {
        self.sessions.iter().map(|s| s.uri.as_str()).collect()
    }

    /// End all sessions.
    pub fn clear(&mut self) {
        self.sessions.clear();
    }
}


// ---------------------------------------------------------------------------
// LinkedEditRangeValidator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LinkedEditRangeValidator {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl LinkedEditRangeValidator {
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

impl Default for LinkedEditRangeValidator {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for LinkedEditRangeValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "LinkedEditRangeValidator({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// LinkedEditUndoIntegration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LinkedEditUndoIntegration {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl LinkedEditUndoIntegration {
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

impl Default for LinkedEditUndoIntegration {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for LinkedEditUndoIntegration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "LinkedEditUndoIntegration({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// LinkedEditRangeValidatorSnapshot — point-in-time snapshot of LinkedEditRangeValidator state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LinkedEditRangeValidatorSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl LinkedEditRangeValidatorSnapshot {
    pub fn capture(source: &LinkedEditRangeValidator, timestamp: u64) -> Self {
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

impl fmt::Display for LinkedEditRangeValidatorSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// LinkedEditUndoIntegrationStats — aggregate statistics for LinkedEditUndoIntegration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct LinkedEditUndoIntegrationStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl LinkedEditUndoIntegrationStats {
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

impl fmt::Display for LinkedEditUndoIntegrationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// LinkedEditRangeValidatorConfig — configuration for LinkedEditRangeValidator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LinkedEditRangeValidatorConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl LinkedEditRangeValidatorConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for LinkedEditRangeValidatorConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for LinkedEditRangeValidatorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// --- LinkedEditGroupV2: group of ranges that edit together ---

pub struct LinkedEditGroupV2 {
    ranges: Vec<(usize, usize)>, // (start, end) offsets
}

impl LinkedEditGroupV2 {
    pub fn new() -> Self { Self { ranges: Vec::new() } }

    pub fn add_range(&mut self, start: usize, end: usize) {
        if start <= end {
            self.ranges.push((start, end));
        }
    }

    pub fn remove_range(&mut self, index: usize) -> bool {
        if index < self.ranges.len() { self.ranges.remove(index); true } else { false }
    }

    pub fn apply_edit(&self, text: &str, old_fragment: &str, new_fragment: &str) -> String {
        let mut result = text.to_string();
        let mut sorted: Vec<(usize, usize)> = self.ranges.clone();
        sorted.sort_by(|a, b| b.0.cmp(&a.0)); // reverse order to preserve offsets
        for (start, end) in sorted {
            if start <= result.len() && end <= result.len() {
                let slice = &result[start..end];
                if slice == old_fragment {
                    result.replace_range(start..end, new_fragment);
                }
            }
        }
        result
    }

    pub fn validate_no_overlap(&self) -> bool {
        let mut sorted = self.ranges.clone();
        sorted.sort_by_key(|r| r.0);
        for w in sorted.windows(2) {
            if w[0].1 > w[1].0 { return false; }
        }
        true
    }

    pub fn range_count(&self) -> usize { self.ranges.len() }

    pub fn sort_ranges(&mut self) {
        self.ranges.sort_by_key(|r| r.0);
    }
}

// --- LinkedEditSessionV2: manage multiple groups ---

pub struct LinkedEditSessionV2 {
    groups: Vec<LinkedEditGroupV2>,
    active: Option<usize>,
}

impl LinkedEditSessionV2 {
    pub fn new() -> Self { Self { groups: Vec::new(), active: None } }

    pub fn add_group(&mut self, group: LinkedEditGroupV2) -> usize {
        let idx = self.groups.len();
        self.groups.push(group);
        idx
    }

    pub fn find_group_at_position(&self, pos: usize) -> Option<usize> {
        self.groups.iter().position(|g| {
            g.ranges.iter().any(|(s, e)| pos >= *s && pos <= *e)
        })
    }

    pub fn active_group(&self) -> Option<&LinkedEditGroupV2> {
        self.active.and_then(|i| self.groups.get(i))
    }

    pub fn set_active(&mut self, index: usize) -> bool {
        if index < self.groups.len() { self.active = Some(index); true } else { false }
    }

    pub fn deactivate(&mut self) { self.active = None; }

    pub fn group_count(&self) -> usize { self.groups.len() }
}

// --- LinkedEditDelta ---

pub struct LinkedEditDelta {
    pub old_text: String,
    pub new_text: String,
    pub offset_shift: isize,
}

impl LinkedEditDelta {
    pub fn new(old_text: &str, new_text: &str) -> Self {
        let shift = new_text.len() as isize - old_text.len() as isize;
        Self { old_text: old_text.to_string(), new_text: new_text.to_string(), offset_shift: shift }
    }

    pub fn apply_to_position(&self, pos: usize) -> usize {
        if self.offset_shift >= 0 {
            pos + self.offset_shift as usize
        } else {
            pos.saturating_sub((-self.offset_shift) as usize)
        }
    }

    pub fn chain_deltas(a: &LinkedEditDelta, b: &LinkedEditDelta) -> LinkedEditDelta {
        LinkedEditDelta {
            old_text: a.old_text.clone(),
            new_text: b.new_text.clone(),
            offset_shift: a.offset_shift + b.offset_shift,
        }
    }

    pub fn inverted_delta(&self) -> LinkedEditDelta {
        LinkedEditDelta {
            old_text: self.new_text.clone(),
            new_text: self.old_text.clone(),
            offset_shift: -self.offset_shift,
        }
    }
}


/// Linked edit configuration manager.
#[derive(Debug, Clone)]
pub struct LinkededitConfig {
    entries: Vec<LinkededitEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single linked edit entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkededitEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl LinkededitEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl LinkededitConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: LinkededitEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&LinkededitEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LinkededitEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&LinkededitEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&LinkededitEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&LinkededitEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<LinkededitEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
        assert_eq!(format!("{}", r), "Ln 1:Col 5 - Ln 3:Col 10");
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

    // ── New functionality tests ──

    #[test]
    fn range_line_count_single_line() {
        let r = LinkedEditingRange::new(3, 0, 3, 10);
        assert_eq!(r.line_count(), 1);
    }

    #[test]
    fn range_line_count_multi_line() {
        let r = LinkedEditingRange::new(1, 0, 5, 10);
        assert_eq!(r.line_count(), 5);
    }

    #[test]
    fn ranges_is_empty() {
        let empty = LinkedEditingRanges::new(vec![], None);
        assert!(empty.is_empty());
        let non_empty = LinkedEditingRanges::new(
            vec![LinkedEditingRange::new(0, 0, 0, 3)],
            None,
        );
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn ranges_has_word_pattern() {
        let without = LinkedEditingRanges::new(vec![], None);
        assert!(!without.has_word_pattern());
        let with = LinkedEditingRanges::new(vec![], Some("ident".into()));
        assert!(with.has_word_pattern());
    }

    #[test]
    fn session_uri_accessor() {
        let session = LinkedEditingSession::new(
            "file:///test.html".into(),
            "text".into(),
            LinkedEditingRanges::new(vec![], None),
        );
        assert_eq!(session.uri(), "file:///test.html");
    }

    #[test]
    fn session_original_text_accessor() {
        let session = LinkedEditingSession::new(
            "f".into(),
            "<div>hello</div>".into(),
            LinkedEditingRanges::new(vec![], None),
        );
        assert_eq!(session.original_text(), "<div>hello</div>");
    }

    #[test]
    fn session_edit_count() {
        let session = LinkedEditingSession::new(
            "f".into(),
            "<div></div>".into(),
            LinkedEditingRanges::new(
                vec![
                    LinkedEditingRange::new(0, 1, 0, 4),
                    LinkedEditingRange::new(0, 7, 0, 10),
                ],
                None,
            ),
        );
        assert_eq!(session.edit_count(), 2);
    }

    #[test]
    fn display_linked_editing_ranges() {
        let r = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 0, 0, 3),
                LinkedEditingRange::new(0, 5, 0, 8),
                LinkedEditingRange::new(1, 0, 1, 4),
            ],
            None,
        );
        assert_eq!(format!("{}", r), "3 linked ranges");
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

    // ---- LinkedEditingUndoStack tests ----

    #[test]
    fn undo_stack_push_and_undo() {
        let mut stack = LinkedEditingUndoStack::new(10);
        stack.push("state1".to_string());
        stack.push("state2".to_string());
        assert!(stack.can_undo());
        let prev = stack.undo("state3").unwrap();
        assert_eq!(prev, "state2");
        let prev2 = stack.undo("state2").unwrap();
        assert_eq!(prev2, "state1");
        assert!(!stack.can_undo());
    }

    #[test]
    fn undo_stack_redo() {
        let mut stack = LinkedEditingUndoStack::new(10);
        stack.push("state1".to_string());
        let prev = stack.undo("state2").unwrap();
        assert_eq!(prev, "state1");
        assert!(stack.can_redo());
        let next = stack.redo("state1").unwrap();
        assert_eq!(next, "state2");
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_push_clears_redo() {
        let mut stack = LinkedEditingUndoStack::new(10);
        stack.push("a".to_string());
        stack.undo("b");
        assert!(stack.can_redo());
        stack.push("c".to_string());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_capacity() {
        let mut stack = LinkedEditingUndoStack::new(2);
        stack.push("a".to_string());
        stack.push("b".to_string());
        stack.push("c".to_string());
        assert_eq!(stack.undo_depth(), 2);
        let prev = stack.undo("d").unwrap();
        assert_eq!(prev, "c");
    }

    // ---- RangeHighlighter tests ----

    #[test]
    fn highlighter_primary_and_secondary() {
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),
            LinkedEditingRange::new(0, 10, 0, 13),
        ];
        let highlights = RangeHighlighter::highlight(&ranges, 0, 2);
        assert_eq!(highlights[0].style, HighlightStyle::Primary);
        assert_eq!(highlights[1].style, HighlightStyle::Secondary);
    }

    #[test]
    fn highlighter_no_active_cursor() {
        let ranges = vec![LinkedEditingRange::new(0, 5, 0, 8)];
        let highlights = RangeHighlighter::highlight(&ranges, 0, 0);
        assert_eq!(highlights[0].style, HighlightStyle::Secondary);
    }

    // ---- LinkedEditValidator tests ----

    #[test]
    fn validator_accepts_valid_edit() {
        let text = "<div>hello</div>";
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),
            LinkedEditingRange::new(0, 12, 0, 15),
        ];
        assert!(LinkedEditValidator::validate(text, &ranges, "span", None).is_ok());
    }

    #[test]
    fn validator_rejects_empty_text() {
        let text = "test";
        let ranges = vec![LinkedEditingRange::new(0, 0, 0, 4)];
        let errs = LinkedEditValidator::validate(text, &ranges, "", None).unwrap_err();
        assert!(errs.contains(&LinkedEditValidationError::EmptyNewText));
    }

    #[test]
    fn validator_rejects_pattern_mismatch() {
        let text = "test";
        let ranges = vec![LinkedEditingRange::new(0, 0, 0, 4)];
        let errs = LinkedEditValidator::validate(text, &ranges, "a b", Some("[a-z]+")).unwrap_err();
        assert!(errs.contains(&LinkedEditValidationError::PatternMismatch));
    }

    // ---- LinkedEditGroup extensions ----

    #[test]
    fn group_clone_with_new_text() {
        let group = LinkedEditGroup::new(vec![(0, 3), (5, 8)], "foo");
        let cloned = group.clone_group(Some("bar"));
        assert_eq!(cloned.current_text, "bar");
        assert_eq!(cloned.ranges, group.ranges);
    }

    #[test]
    fn group_split_at() {
        let group = LinkedEditGroup::new(vec![(0, 3), (5, 8), (10, 13)], "foo");
        let (left, right) = group.split_at(1);
        assert_eq!(left.len(), 1);
        assert_eq!(right.len(), 2);
        assert_eq!(left.current_text, "foo");
    }

    #[test]
    fn total_span_single_line_ranges() {
        let ranges = vec![
            LinkedEditingRange::new(0, 0, 0, 5),
            LinkedEditingRange::new(0, 10, 0, 15),
        ];
        assert_eq!(total_span(&ranges), Some(10));
    }

    #[test]
    fn total_span_multi_line_returns_none() {
        let ranges = vec![LinkedEditingRange::new(0, 0, 1, 5)];
        assert_eq!(total_span(&ranges), None);
    }

    #[test]
    fn total_span_empty() {
        assert_eq!(total_span(&[]), Some(0));
    }

    #[test]
    fn ranges_on_line_filters_correctly() {
        let ranges = vec![
            LinkedEditingRange::new(0, 0, 0, 5),
            LinkedEditingRange::new(1, 0, 1, 3),
            LinkedEditingRange::new(2, 0, 2, 4),
        ];
        let on1 = ranges_on_line(&ranges, 1);
        assert_eq!(on1.len(), 1);
        assert_eq!(on1[0].start_col, 0);
        assert_eq!(on1[0].end_col, 3);
    }

    #[test]
    fn ranges_on_line_empty_input() {
        assert!(ranges_on_line(&[], 5).is_empty());
    }

    #[test]
    fn shift_ranges_positive_delta() {
        let ranges = vec![LinkedEditingRange::new(1, 0, 1, 5)];
        let shifted = shift_ranges(&ranges, 3);
        assert_eq!(shifted[0].start_line, 4);
        assert_eq!(shifted[0].end_line, 4);
    }

    #[test]
    fn shift_ranges_negative_clamps_to_zero() {
        let ranges = vec![LinkedEditingRange::new(1, 0, 1, 5)];
        let shifted = shift_ranges(&ranges, -10);
        assert_eq!(shifted[0].start_line, 0);
        assert_eq!(shifted[0].end_line, 0);
    }

    #[test]
    fn ranges_equal_identical() {
        let a = vec![LinkedEditingRange::new(0, 0, 0, 5)];
        let b = vec![LinkedEditingRange::new(0, 0, 0, 5)];
        assert!(ranges_equal(&a, &b));
    }

    #[test]
    fn ranges_equal_different_lengths() {
        let a = vec![LinkedEditingRange::new(0, 0, 0, 5)];
        let b: Vec<LinkedEditingRange> = vec![];
        assert!(!ranges_equal(&a, &b));
    }

    #[test]
    fn deduplicate_ranges_removes_dupes() {
        let ranges = vec![
            LinkedEditingRange::new(0, 0, 0, 5),
            LinkedEditingRange::new(0, 0, 0, 5),
            LinkedEditingRange::new(1, 0, 1, 3),
        ];
        let deduped = deduplicate_ranges(&ranges);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn deduplicate_ranges_empty() {
        assert!(deduplicate_ranges(&[]).is_empty());
    }

    #[test]
    fn bounding_range_single() {
        let ranges = vec![LinkedEditingRange::new(2, 5, 4, 10)];
        let b = bounding_range(&ranges).unwrap();
        assert_eq!(b.start_line, 2);
        assert_eq!(b.end_line, 4);
    }

    #[test]
    fn bounding_range_multiple() {
        let ranges = vec![
            LinkedEditingRange::new(0, 3, 0, 8),
            LinkedEditingRange::new(5, 0, 7, 2),
        ];
        let b = bounding_range(&ranges).unwrap();
        assert_eq!(b.start_line, 0);
        assert_eq!(b.start_col, 3);
        assert_eq!(b.end_line, 7);
        assert_eq!(b.end_col, 2);
    }

    #[test]
    fn bounding_range_empty() {
        assert!(bounding_range(&[]).is_none());
    }

    // ---- propagate_ranges tests ----

    #[test]
    fn propagate_ranges_updates_positions() {
        let text = "<div>hello</div>";
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),   // "div" at byte 1..4
            LinkedEditingRange::new(0, 12, 0, 15),  // "div" at byte 12..15
        ];
        let new_ranges = propagate_ranges(text, &ranges, "span").unwrap();
        assert_eq!(new_ranges.len(), 2);
        // After replacing "div" with "span", first range should be at 1..5
        assert_eq!(new_ranges[0], LinkedEditingRange::new(0, 1, 0, 5));
        // Second range shifts right by 1 (first replacement grew by 1 char)
        // Original was 12..15, now "span" is 4 chars => 13..17
        assert_eq!(new_ranges[1], LinkedEditingRange::new(0, 13, 0, 17));
    }

    #[test]
    fn propagate_ranges_shrinking() {
        let text = "<section></section>";
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 8),    // "section"
            LinkedEditingRange::new(0, 11, 0, 18),   // "section"
        ];
        let new_ranges = propagate_ranges(text, &ranges, "p").unwrap();
        // "section" (7) -> "p" (1), delta = -6
        assert_eq!(new_ranges[0], LinkedEditingRange::new(0, 1, 0, 2));
        assert_eq!(new_ranges[1], LinkedEditingRange::new(0, 5, 0, 6));
    }

    #[test]
    fn propagate_ranges_empty_ranges() {
        let result = propagate_ranges("text", &[], "x").unwrap();
        assert!(result.is_empty());
    }

    // ---- detect_conflicts tests ----

    #[test]
    fn detect_conflicts_no_conflict() {
        let groups = vec![
            LinkedEditGroup::new(vec![(0, 3), (10, 13)], "foo"),
            LinkedEditGroup::new(vec![(5, 8), (15, 18)], "bar"),
        ];
        let conflicts = detect_conflicts(&groups);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn detect_conflicts_with_overlap() {
        let groups = vec![
            LinkedEditGroup::new(vec![(0, 5)], "hello"),
            LinkedEditGroup::new(vec![(3, 8)], "world"),
        ];
        let conflicts = detect_conflicts(&groups);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].group_a, 0);
        assert_eq!(conflicts[0].group_b, 1);
        assert_eq!(conflicts[0].overlapping_pairs, vec![(0, 0)]);
    }

    // ---- cursor mapping tests ----

    #[test]
    fn map_cursor_to_linked_ranges_basic() {
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),   // first "div"
            LinkedEditingRange::new(2, 2, 2, 5),   // second "div"
        ];
        // Cursor in first range at col 3 => offset 2 within range
        let positions = map_cursor_to_linked_ranges(&ranges, 0, 3).unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], (0, 3));  // source stays same
        assert_eq!(positions[1], (2, 4));  // offset 2 applied to second range
    }

    #[test]
    fn map_cursor_outside_ranges_returns_none() {
        let ranges = vec![LinkedEditingRange::new(0, 5, 0, 10)];
        assert!(map_cursor_to_linked_ranges(&ranges, 0, 0).is_none());
    }

    #[test]
    fn map_cursor_with_metadata_returns_info() {
        let ranges = vec![
            LinkedEditingRange::new(0, 1, 0, 4),
            LinkedEditingRange::new(0, 10, 0, 13),
        ];
        let (mapping, positions) = map_cursor_with_metadata(&ranges, 0, 2).unwrap();
        assert_eq!(mapping.source_range, 0);
        assert_eq!(mapping.offset_in_range, 1);
        assert_eq!(positions[1], (0, 11));
    }

    // ---- range_intersection tests ----

    #[test]
    fn range_intersection_overlapping() {
        let a = LinkedEditingRange::new(0, 2, 0, 8);
        let b = LinkedEditingRange::new(0, 5, 0, 12);
        let inter = range_intersection(&a, &b).unwrap();
        assert_eq!(inter, LinkedEditingRange::new(0, 5, 0, 8));
    }

    #[test]
    fn range_intersection_no_overlap() {
        let a = LinkedEditingRange::new(0, 0, 0, 3);
        let b = LinkedEditingRange::new(0, 5, 0, 8);
        assert!(range_intersection(&a, &b).is_none());
    }

    // ---- ranges_containing_position tests ----

    #[test]
    fn ranges_containing_position_multiple() {
        let ranges = vec![
            LinkedEditingRange::new(0, 0, 0, 10),
            LinkedEditingRange::new(0, 5, 0, 15),
        ];
        let found = ranges_containing_position(&ranges, 0, 7);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, 0);
        assert_eq!(found[1].0, 1);
    }

    // ---- LinkedEditingCoordinator tests ----

    #[test]
    fn coordinator_start_and_find_session() {
        let mut coord = LinkedEditingCoordinator::new();
        let ranges = LinkedEditingRanges::new(
            vec![LinkedEditingRange::new(0, 1, 0, 4)],
            None,
        );
        coord.start_session("file:///a.html".into(), "<div>".into(), ranges);
        assert_eq!(coord.session_count(), 1);
        assert!(coord.session_for("file:///a.html").is_some());
        assert!(coord.session_for("file:///b.html").is_none());
    }

    #[test]
    fn coordinator_end_session() {
        let mut coord = LinkedEditingCoordinator::new();
        let ranges = LinkedEditingRanges::new(vec![], None);
        coord.start_session("file:///a.html".into(), "".into(), ranges);
        assert!(coord.end_session("file:///a.html"));
        assert_eq!(coord.session_count(), 0);
        assert!(!coord.end_session("file:///a.html"));
    }

    #[test]
    fn coordinator_replaces_existing_session() {
        let mut coord = LinkedEditingCoordinator::new();
        let r1 = LinkedEditingRanges::new(
            vec![LinkedEditingRange::new(0, 0, 0, 3)],
            None,
        );
        let r2 = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 0, 0, 4),
                LinkedEditingRange::new(0, 6, 0, 10),
            ],
            None,
        );
        coord.start_session("file:///a.html".into(), "old".into(), r1);
        coord.start_session("file:///a.html".into(), "new".into(), r2);
        assert_eq!(coord.session_count(), 1);
        let s = coord.session_for("file:///a.html").unwrap();
        assert_eq!(s.range_count(), 2);
        assert_eq!(s.original_text(), "new");
    }

    #[test]
    fn coordinator_active_uris() {
        let mut coord = LinkedEditingCoordinator::new();
        let r = LinkedEditingRanges::new(vec![], None);
        coord.start_session("file:///a.html".into(), "".into(), r.clone());
        coord.start_session("file:///b.html".into(), "".into(), r);
        let uris = coord.active_uris();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&"file:///a.html"));
        assert!(uris.contains(&"file:///b.html"));
    }

    #[test]
    fn coordinator_clear() {
        let mut coord = LinkedEditingCoordinator::new();
        let r = LinkedEditingRanges::new(vec![], None);
        coord.start_session("file:///a.html".into(), "".into(), r);
        coord.clear();
        assert_eq!(coord.session_count(), 0);
    }

    #[test]
    fn coordinator_mutable_session_update() {
        let mut coord = LinkedEditingCoordinator::new();
        let ranges = LinkedEditingRanges::new(
            vec![
                LinkedEditingRange::new(0, 1, 0, 4),
                LinkedEditingRange::new(0, 12, 0, 15),
            ],
            None,
        );
        coord.start_session(
            "file:///a.html".into(),
            "<div>hello</div>".into(),
            ranges,
        );
        let session = coord.session_for_mut("file:///a.html").unwrap();
        let result = session.update("span").unwrap();
        assert_eq!(result, "<span>hello</span>");
    }

    #[test] fn linkedEditRangeValidator_new() { let s = LinkedEditRangeValidator::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn linkedEditRangeValidator_add() { let mut s = LinkedEditRangeValidator::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn linkedEditRangeValidator_remove() { let mut s = LinkedEditRangeValidator::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn linkedEditRangeValidator_config() { let mut s = LinkedEditRangeValidator::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn linkedEditRangeValidator_nav() { let mut s = LinkedEditRangeValidator::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn linkedEditRangeValidator_filter() { let mut s = LinkedEditRangeValidator::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn linkedEditRangeValidator_display() { assert!(format!("{}", LinkedEditRangeValidator::new()).contains("LinkedEditRangeValidator")); }
    #[test] fn linkedEditUndoIntegration_new() { let s = LinkedEditUndoIntegration::new(); assert!(s.is_empty()); }
    #[test] fn linkedEditUndoIntegration_add() { let mut s = LinkedEditUndoIntegration::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn linkedEditUndoIntegration_active() { let mut s = LinkedEditUndoIntegration::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn linkedEditUndoIntegration_error() { let mut s = LinkedEditUndoIntegration::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn linkedEditUndoIntegration_rm_group() { let mut s = LinkedEditUndoIntegration::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn linkedEditUndoIntegration_display() { assert!(format!("{}", LinkedEditUndoIntegration::new()).contains("LinkedEditUndoIntegration")); }


    #[test] fn linkedEditRangeValidator_snap_capture() {
        let s = LinkedEditRangeValidator::new();
        let snap = LinkedEditRangeValidatorSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn linkedEditRangeValidator_snap_stale() {
        let s = LinkedEditRangeValidator::new();
        let snap = LinkedEditRangeValidatorSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn linkedEditRangeValidator_snap_diff() {
        let s = LinkedEditRangeValidator::new();
        let s1v = LinkedEditRangeValidatorSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn linkedEditRangeValidator_snap_display() {
        let s = LinkedEditRangeValidator::new();
        let snap = LinkedEditRangeValidatorSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn linkedEditUndoIntegration_stats_record() {
        let mut st = LinkedEditUndoIntegrationStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn linkedEditUndoIntegration_stats_hit_ratio() {
        let mut st = LinkedEditUndoIntegrationStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn linkedEditUndoIntegration_stats_merge() {
        let mut a = LinkedEditUndoIntegrationStats::new();
        a.total_adds = 5;
        let mut b = LinkedEditUndoIntegrationStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn linkedEditUndoIntegration_stats_display() {
        let st = LinkedEditUndoIntegrationStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn linkedEditRangeValidator_config_default() {
        let c = LinkedEditRangeValidatorConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn linkedEditRangeValidator_config_builder() {
        let c = LinkedEditRangeValidatorConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn linkedEditRangeValidator_config_labels() {
        let mut c = LinkedEditRangeValidatorConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn linkedEditRangeValidator_config_cleanup_threshold() {
        let c = LinkedEditRangeValidatorConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn linkedEditRangeValidator_config_display() {
        assert!(format!("{}", LinkedEditRangeValidatorConfig::new()).contains("Config"));
    }
    #[test] fn linkedEditUndoIntegration_stats_peaks() {
        let mut st = LinkedEditUndoIntegrationStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    #[test]
    fn linked_edit_group_v2_add_range() {
        let mut g = LinkedEditGroupV2::new();
        g.add_range(0, 3);
        g.add_range(7, 10);
        assert_eq!(g.range_count(), 2);
    }

    #[test]
    fn linked_edit_group_v2_remove_range() {
        let mut g = LinkedEditGroupV2::new();
        g.add_range(0, 3);
        assert!(g.remove_range(0));
        assert_eq!(g.range_count(), 0);
    }

    #[test]
    fn linked_edit_group_v2_no_overlap() {
        let mut g = LinkedEditGroupV2::new();
        g.add_range(0, 3);
        g.add_range(5, 8);
        assert!(g.validate_no_overlap());
    }

    #[test]
    fn linked_edit_group_v2_has_overlap() {
        let mut g = LinkedEditGroupV2::new();
        g.add_range(0, 5);
        g.add_range(3, 8);
        assert!(!g.validate_no_overlap());
    }

    #[test]
    fn linked_edit_group_v2_apply_edit() {
        let mut g = LinkedEditGroupV2::new();
        g.add_range(1, 4);
        g.add_range(12, 15);
        let result = g.apply_edit("<div>hello</div>", "div", "span");
        assert_eq!(result, "<span>hello</span>");
    }

    #[test]
    fn linked_edit_session_v2_add_group() {
        let mut s = LinkedEditSessionV2::new();
        let g = LinkedEditGroupV2::new();
        s.add_group(g);
        assert_eq!(s.group_count(), 1);
    }

    #[test]
    fn linked_edit_session_v2_find_group() {
        let mut s = LinkedEditSessionV2::new();
        let mut g = LinkedEditGroupV2::new();
        g.add_range(5, 10);
        s.add_group(g);
        assert_eq!(s.find_group_at_position(7), Some(0));
        assert_eq!(s.find_group_at_position(20), None);
    }

    #[test]
    fn linked_edit_session_v2_active() {
        let mut s = LinkedEditSessionV2::new();
        s.add_group(LinkedEditGroupV2::new());
        assert!(s.active_group().is_none());
        s.set_active(0);
        assert!(s.active_group().is_some());
        s.deactivate();
        assert!(s.active_group().is_none());
    }

    #[test]
    fn linked_edit_delta_offset_shift() {
        let d = LinkedEditDelta::new("ab", "abcd");
        assert_eq!(d.offset_shift, 2);
    }

    #[test]
    fn linked_edit_delta_apply_to_position() {
        let d = LinkedEditDelta::new("hi", "hello");
        assert_eq!(d.apply_to_position(10), 13);
    }

    #[test]
    fn linked_edit_delta_inverted() {
        let d = LinkedEditDelta::new("old", "newer");
        let inv = d.inverted_delta();
        assert_eq!(inv.old_text, "newer");
        assert_eq!(inv.new_text, "old");
        assert_eq!(inv.offset_shift, -d.offset_shift);
    }

    #[test]
    fn linked_edit_delta_chain() {
        let a = LinkedEditDelta::new("a", "ab");
        let b = LinkedEditDelta::new("ab", "abcd");
        let chained = LinkedEditDelta::chain_deltas(&a, &b);
        assert_eq!(chained.old_text, "a");
        assert_eq!(chained.new_text, "abcd");
        assert_eq!(chained.offset_shift, 3);
    }


    #[test]
    fn linkededit_entry_creation() {
        let e = LinkededitEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn linkededit_entry_with_priority() {
        let e = LinkededitEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn linkededit_entry_metadata() {
        let e = LinkededitEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn linkededit_entry_remove_meta() {
        let mut e = LinkededitEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn linkededit_entry_activate_deactivate() {
        let mut e = LinkededitEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn linkededit_config_add_sorted() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("lo", "Lo").with_priority(1));
        c.add(LinkededitEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn linkededit_config_capacity() {
        let mut c = LinkededitConfig::new(1);
        assert!(c.add(LinkededitEntry::new("a", "A")));
        assert!(!c.add(LinkededitEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn linkededit_config_remove() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn linkededit_config_get() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn linkededit_config_active_entries() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "A"));
        c.add(LinkededitEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn linkededit_config_enable_disable() {
        let mut c = LinkededitConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn linkededit_config_clear() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn linkededit_config_find_by_label() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn linkededit_config_top_n() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "A").with_priority(1));
        c.add(LinkededitEntry::new("b", "B").with_priority(2));
        c.add(LinkededitEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn linkededit_config_deactivate_activate_all() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "A"));
        c.add(LinkededitEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn linkededit_config_highest_priority() {
        let mut c = LinkededitConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(LinkededitEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn linkededit_config_contains() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn linkededit_config_labels() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "Alpha"));
        c.add(LinkededitEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn linkededit_config_drain_inactive() {
        let mut c = LinkededitConfig::new(10);
        c.add(LinkededitEntry::new("a", "A"));
        c.add(LinkededitEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }

}
