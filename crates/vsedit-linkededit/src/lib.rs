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
// xa_ extended helpers for linkededit
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaLinkededitRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaLinkededitRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaLinkededitCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaLinkededitCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaLinkededitCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 113
// ---------------------------------------------------------------------------

/// Generic object pool `Xc113Pool<T>`.
pub struct Xc113Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc113Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc113PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc113Pool<T> {
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
    pub fn stats(&self) -> Xc113PoolStats {
        Xc113PoolStats {
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

impl<T> Default for Xc113Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc113Scheduler`.
pub struct Xc113Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc113Scheduler {
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

impl Default for Xc113Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_113 hash for the given byte slice.
pub fn xc_113_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_113 convention.
pub fn xc_113_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_40 deepening: state machine + event bus ---

/// States for the Xd40 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd40State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd40State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd40Transition {
    pub from: Xd40State,
    pub to: Xd40State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd40StateMachine {
    current: Xd40State,
    history: Vec<Xd40Transition>,
    step_counter: usize,
}

impl Xd40StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd40State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd40State {
        self.current
    }

    pub fn history(&self) -> &[Xd40Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd40State) -> Result<Xd40State, String> {
        let allowed = match (self.current, target) {
            (Xd40State::Idle, Xd40State::Running) => true,
            (Xd40State::Running, Xd40State::Paused) => true,
            (Xd40State::Running, Xd40State::Done) => true,
            (Xd40State::Paused, Xd40State::Running) => true,
            (Xd40State::Paused, Xd40State::Done) => true,
            (Xd40State::Done, Xd40State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_40: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd40Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd40SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd40State> {
        let prefix = "Xd40SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd40State::Idle),
            "Running" => Some(Xd40State::Running),
            "Paused" => Some(Xd40State::Paused),
            "Done" => Some(Xd40State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd40State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd40 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd40Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd40Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd40HandlerFn = Box<dyn Fn(&Xd40Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd40EventBus {
    handlers: Vec<(usize, Option<String>, Xd40HandlerFn)>,
    next_id: usize,
    published: Vec<Xd40Event>,
}

impl Xd40EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd40Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd40Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd40Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd40Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #38
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf38Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf38TrieNode {
    children: std::collections::HashMap<char, Xf38TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf38Trie {
    root: Xf38TrieNode,
    count: usize,
}

impl Xf38Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf38TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf38TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf38TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf38BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf38BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 112).
pub struct Xh112SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh112SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 154 as u64,
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

/// A compact bit set supporting boolean operations (variant 112).
pub struct Xh112BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh112BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 112).
pub struct Xi112Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi112Deque<T> {
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
pub struct Xi112Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi112Interval {
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

/// A simple interval tree (variant 112).
pub struct Xi112IntervalTree {
    xi_intervals: Vec<Xi112Interval>,
}

impl Xi112IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi112Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi112Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi112Interval) -> Vec<&Xi112Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi112Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi112Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi112Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi112Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi112Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi112Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 112) ---

/// Disjoint set / union-find for crate 112.
pub struct Xj112UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj112UnionFind {
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

const XJ112_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 112.
pub struct Xj112BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj112BTreeNode<K, V>>>,
    len: usize,
}

struct Xj112BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj112BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj112BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ112_BTREE_ORDER - 1
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
        let mid = XJ112_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj112BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj112BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj112BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj112BTreeNode::xj_new_leaf();
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


// --- xk_112 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk112SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk112SegmentTree {
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
pub struct Xk112DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk112DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_112).
#[derive(Debug, Clone)]
pub struct Xl112Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl112Rope {
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

/// Suffix array for efficient string searching (xl_112).
#[derive(Debug, Clone)]
pub struct Xl112SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl112SuffixArray {
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


    // xa_ extended tests for linkededit
    #[test]
    fn xa_linkededit_ring_new() {
        let rb = super::XaLinkededitRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_linkededit_ring_push_len() {
        let mut rb = super::XaLinkededitRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_linkededit_ring_wrap() {
        let mut rb = super::XaLinkededitRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_linkededit_ring_mean_empty() {
        let rb = super::XaLinkededitRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_linkededit_ring_mean_values() {
        let mut rb = super::XaLinkededitRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_linkededit_ring_min_max() {
        let mut rb = super::XaLinkededitRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_linkededit_ring_iter() {
        let mut rb = super::XaLinkededitRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_linkededit_counter_new() {
        let c = super::XaLinkededitCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_linkededit_counter_inc() {
        let mut c = super::XaLinkededitCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_linkededit_counter_inc_by() {
        let mut c = super::XaLinkededitCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_linkededit_counter_reset() {
        let mut c = super::XaLinkededitCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_linkededit_counter_clear() {
        let mut c = super::XaLinkededitCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_linkededit_counter_default() {
        let c = super::XaLinkededitCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 113 ----

    #[test]
    fn xc_113_pool_new_empty() {
        let pool: super::Xc113Pool<i32> = super::Xc113Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_113_pool_release_acquire() {
        let mut pool = super::Xc113Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_113_pool_acquire_empty() {
        let mut pool: super::Xc113Pool<i32> = super::Xc113Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_113_pool_full() {
        let mut pool = super::Xc113Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_113_pool_drain() {
        let mut pool = super::Xc113Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_113_pool_stats() {
        let mut pool = super::Xc113Pool::new(8);
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
    fn xc_113_pool_clear() {
        let mut pool = super::Xc113Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_113_pool_shrink() {
        let mut pool = super::Xc113Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_113_pool_default() {
        let pool: super::Xc113Pool<String> = super::Xc113Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_113_pool_extend() {
        let mut pool = super::Xc113Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_113_pool_retain() {
        let mut pool = super::Xc113Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_113_scheduler_round_robin() {
        let mut sched = super::Xc113Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_113_scheduler_empty() {
        let mut sched = super::Xc113Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_113_scheduler_reset() {
        let mut sched = super::Xc113Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_113_scheduler_add_remove() {
        let mut sched = super::Xc113Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_113_scheduler_targets() {
        let sched = super::Xc113Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_113_hash_empty() {
        assert_eq!(super::xc_113_hash(b""), 5381);
    }

    #[test]
    fn xc_113_hash_data() {
        let h = super::xc_113_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_113_hash(b"hello"), h);
    }

    #[test]
    fn xc_113_reverse_str() {
        assert_eq!(super::xc_113_reverse("abc"), "cba");
        assert_eq!(super::xc_113_reverse(""), "");
    }


    // --- xd_40 deepening tests ---

    #[test]
    fn xd_40_sm_initial_state() {
        let sm = Xd40StateMachine::new();
        assert_eq!(sm.current_state(), Xd40State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_40_sm_valid_idle_to_running() {
        let mut sm = Xd40StateMachine::new();
        assert!(sm.transition(Xd40State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd40State::Running);
    }

    #[test]
    fn xd_40_sm_valid_running_to_paused() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        assert!(sm.transition(Xd40State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd40State::Paused);
    }

    #[test]
    fn xd_40_sm_valid_running_to_done() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        assert!(sm.transition(Xd40State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd40State::Done);
    }

    #[test]
    fn xd_40_sm_valid_paused_to_running() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        sm.transition(Xd40State::Paused).unwrap();
        assert!(sm.transition(Xd40State::Running).is_ok());
    }

    #[test]
    fn xd_40_sm_valid_done_to_idle() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        sm.transition(Xd40State::Done).unwrap();
        assert!(sm.transition(Xd40State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd40State::Idle);
    }

    #[test]
    fn xd_40_sm_invalid_idle_to_done() {
        let mut sm = Xd40StateMachine::new();
        assert!(sm.transition(Xd40State::Done).is_err());
    }

    #[test]
    fn xd_40_sm_invalid_idle_to_paused() {
        let mut sm = Xd40StateMachine::new();
        assert!(sm.transition(Xd40State::Paused).is_err());
    }

    #[test]
    fn xd_40_sm_history_tracking() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        sm.transition(Xd40State::Paused).unwrap();
        sm.transition(Xd40State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd40State::Idle);
        assert_eq!(sm.history()[0].to, Xd40State::Running);
        assert_eq!(sm.history()[1].from, Xd40State::Running);
        assert_eq!(sm.history()[2].to, Xd40State::Done);
    }

    #[test]
    fn xd_40_sm_serialize_deserialize() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd40StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd40State::Running));
    }

    #[test]
    fn xd_40_sm_deserialize_invalid() {
        assert_eq!(Xd40StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_40_sm_reset() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd40State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_40_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd40EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd40Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_40_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd40EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd40Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd40Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_40_bus_unsubscribe() {
        let mut bus = Xd40EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_40_event_kind_and_payload() {
        let e = Xd40Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd40Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_40_bus_clear_history() {
        let mut bus = Xd40EventBus::new();
        bus.publish(Xd40Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_40_sm_step_counter_increments() {
        let mut sm = Xd40StateMachine::new();
        sm.transition(Xd40State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd40State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #38 --

    #[test]
    fn xf38_trie_insert_search() {
        let mut t = Xf38Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf38_trie_starts_with() {
        let mut t = Xf38Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf38_trie_remove() {
        let mut t = Xf38Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf38_trie_word_count() {
        let mut t = Xf38Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf38_trie_longest_prefix() {
        let mut t = Xf38Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf38_trie_all_words() {
        let mut t = Xf38Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf38_trie_autocomplete() {
        let mut t = Xf38Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf38_trie_empty_search() {
        let t = Xf38Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf38_bloom_add_contains() {
        let mut bf = Xf38BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf38_bloom_probably_absent() {
        let bf = Xf38BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf38_bloom_false_positive_rate() {
        let mut bf = Xf38BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf38_bloom_clear() {
        let mut bf = Xf38BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf38_bloom_union() {
        let mut a = Xf38BloomFilter::xf_new(512, 2);
        let mut b = Xf38BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf38_bloom_intersection_estimate() {
        let mut a = Xf38BloomFilter::xf_new(512, 2);
        let mut b = Xf38BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf38_bloom_union_size_mismatch() {
        let a = Xf38BloomFilter::xf_new(256, 2);
        let b = Xf38BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh112_skip_insert_contains() {
        let mut sl = super::Xh112SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh112_skip_remove() {
        let mut sl = super::Xh112SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh112_skip_len() {
        let mut sl = super::Xh112SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh112_skip_range_query() {
        let mut sl = super::Xh112SkipList::xh_new(4);
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
    fn xh112_skip_floor_ceiling() {
        let mut sl = super::Xh112SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh112_skip_rank() {
        let mut sl = super::Xh112SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh112_skip_empty() {
        let sl = super::Xh112SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh112_skip_duplicates() {
        let mut sl = super::Xh112SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh112_bitset_set_test() {
        let mut bs = super::Xh112BitSet::xh_new(256);
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
    fn xh112_bitset_clear_count() {
        let mut bs = super::Xh112BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh112_bitset_and_or_xor() {
        let mut a = super::Xh112BitSet::xh_new(128);
        let mut b = super::Xh112BitSet::xh_new(128);
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
    fn xh112_bitset_iter_ones() {
        let mut bs = super::Xh112BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh112_bitset_first_last() {
        let mut bs = super::Xh112BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh112_bitset_empty() {
        let bs = super::Xh112BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi112_deque_push_pop_back() {
        let mut dq = super::Xi112Deque::xi_new(4);
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
    fn xi112_deque_push_pop_front() {
        let mut dq = super::Xi112Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi112_deque_mixed_ops() {
        let mut dq = super::Xi112Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi112_deque_get_and_split() {
        let mut dq = super::Xi112Deque::xi_new(8);
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
    fn xi112_deque_rotate_left() {
        let mut dq = super::Xi112Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi112_deque_rotate_right() {
        let mut dq = super::Xi112Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi112_deque_grow() {
        let mut dq = super::Xi112Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi112_deque_empty() {
        let dq = super::Xi112Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi112_interval_tree_insert_query() {
        let mut tree = super::Xi112IntervalTree::xi_new();
        tree.xi_insert(super::Xi112Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi112Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi112Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi112_interval_tree_overlap() {
        let mut tree = super::Xi112IntervalTree::xi_new();
        tree.xi_insert(super::Xi112Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi112Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi112Interval::xi_new(12, 20));
        let q = super::Xi112Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi112_interval_tree_remove() {
        let mut tree = super::Xi112IntervalTree::xi_new();
        tree.xi_insert(super::Xi112Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi112Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi112_interval_tree_gaps() {
        let mut tree = super::Xi112IntervalTree::xi_new();
        tree.xi_insert(super::Xi112Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi112Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi112Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi112Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi112Interval::xi_new(8, 10));
    }

    #[test]
    fn xi112_interval_tree_merge() {
        let mut tree = super::Xi112IntervalTree::xi_new();
        tree.xi_insert(super::Xi112Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi112Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi112Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi112Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi112Interval::xi_new(10, 15));
    }

    #[test]
    fn xi112_interval_tree_all() {
        let mut tree = super::Xi112IntervalTree::xi_new();
        tree.xi_insert(super::Xi112Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi112Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi112_interval_tree_empty() {
        let tree = super::Xi112IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi112_interval_tree_contains_point() {
        let iv = super::Xi112Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 112) ---

    #[test]
    fn xj_112_uf_make_and_find() {
        let mut uf = super::Xj112UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_112_uf_union_connected() {
        let mut uf = super::Xj112UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_112_uf_component_count() {
        let mut uf = super::Xj112UnionFind::xj_new();
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
    fn xj_112_uf_component_size() {
        let mut uf = super::Xj112UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_112_uf_largest_component() {
        let mut uf = super::Xj112UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_112_uf_many_elements() {
        let mut uf = super::Xj112UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_112_uf_separate_components() {
        let mut uf = super::Xj112UnionFind::xj_new();
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
    fn xj_112_uf_path_compression() {
        let mut uf = super::Xj112UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_112_bt_insert_get() {
        let mut bt = super::Xj112BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_112_bt_contains_len() {
        let mut bt = super::Xj112BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_112_bt_replace() {
        let mut bt = super::Xj112BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_112_bt_remove() {
        let mut bt = super::Xj112BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_112_bt_keys_values() {
        let mut bt = super::Xj112BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_112_bt_range() {
        let mut bt = super::Xj112BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_112_bt_min_max() {
        let mut bt = super::Xj112BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_112_bt_many_inserts() {
        let mut bt = super::Xj112BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_112 segment tree tests ---

    #[test]
    fn xk_112_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk112SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_112_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk112SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_112_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk112SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_112_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk112SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_112_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk112SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_112_st_single_element() {
        let data = vec![42];
        let st = super::Xk112SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_112_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk112SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_112_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk112SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_112 disjoint intervals tests ---

    #[test]
    fn xk_112_di_add_and_count() {
        let mut di = super::Xk112DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_112_di_merge_overlap() {
        let mut di = super::Xk112DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_112_di_contains() {
        let mut di = super::Xk112DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_112_di_remove() {
        let mut di = super::Xk112DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_112_di_covered_length() {
        let mut di = super::Xk112DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_112_di_gaps() {
        let mut di = super::Xk112DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_112_di_merge_adjacent() {
        let mut di = super::Xk112DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_112_di_empty() {
        let di = super::Xk112DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_112_rope_new_empty() {
        let rope = super::Xl112Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_112_rope_from_str() {
        let rope = super::Xl112Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_112_rope_insert_at() {
        let mut rope = super::Xl112Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_112_rope_delete_range() {
        let mut rope = super::Xl112Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_112_rope_char_at() {
        let rope = super::Xl112Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_112_rope_split_concat() {
        let rope = super::Xl112Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_112_rope_line_count() {
        let rope = super::Xl112Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_112_rope_line_at() {
        let rope = super::Xl112Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_112_sa_build_and_search() {
        let sa = super::Xl112SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_112_sa_count() {
        let sa = super::Xl112SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_112_sa_longest_repeated() {
        let sa = super::Xl112SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_112_sa_all_positions() {
        let sa = super::Xl112SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_112_sa_len() {
        let sa = super::Xl112SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_112_sa_empty() {
        let sa = super::Xl112SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_112_rope_slice() {
        let rope = super::Xl112Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_112_sa_search_start() {
        let sa = super::Xl112SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
