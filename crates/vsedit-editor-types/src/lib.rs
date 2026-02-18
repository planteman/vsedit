//! Position, Range, Selection types.
//!
//! Core editor coordinate types equivalent to VS Code's
//! `vs/editor/common/core/position.ts`, `range.ts`, `selection.ts`.

use std::fmt;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// A 1-based line and column coordinate in the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
}

impl Position {
    /// Create a new position. Both `line` and `column` are 1-based.
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    /// Returns `true` if this position is strictly before `other`.
    pub fn is_before(&self, other: &Position) -> bool {
        self < other
    }

    /// Returns `true` if this position is before or equal to `other`.
    pub fn is_before_or_equal(&self, other: &Position) -> bool {
        self <= other
    }

    /// Returns `true` if this position is strictly after `other`.
    pub fn is_after(&self, other: &Position) -> bool {
        self > other
    }

    /// Returns `true` if this position is after or equal to `other`.
    pub fn is_after_or_equal(&self, other: &Position) -> bool {
        self >= other
    }

    /// Returns `true` if this position equals `other`.
    pub fn equals(&self, other: &Position) -> bool {
        self == other
    }

    /// Returns the smaller of two positions.
    pub fn min(a: Position, b: Position) -> Position {
        if a <= b { a } else { b }
    }

    /// Returns the larger of two positions.
    pub fn max(a: Position, b: Position) -> Position {
        if a >= b { a } else { b }
    }

    /// Factory for the origin position (0, 0).
    pub fn origin() -> Position {
        Position { line: 0, column: 0 }
    }

    /// Create a new position by translating this one by the given deltas.
    ///
    /// Negative deltas move the position upward (line) or leftward (column).
    /// The result is clamped so that neither component goes below zero.
    pub fn translate(&self, line_delta: i32, column_delta: i32) -> Position {
        let new_line = (self.line as i64 + line_delta as i64).max(0) as u32;
        let new_column = (self.column as i64 + column_delta as i64).max(0) as u32;
        Position {
            line: new_line,
            column: new_column,
        }
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line
            .cmp(&other.line)
            .then(self.column.cmp(&other.column))
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ln {}, Col {}", self.line, self.column)
    }
}

// ---------------------------------------------------------------------------
// Range
// ---------------------------------------------------------------------------

/// A range in the editor defined by a start and end `Position`.
///
/// The start position is inclusive and the end position is exclusive,
/// following VS Code conventions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl Range {
    /// Create a range from individual line/column values.
    pub fn new(start_line: u32, start_column: u32, end_line: u32, end_column: u32) -> Self {
        Self::from_positions(
            Position::new(start_line, start_column),
            Position::new(end_line, end_column),
        )
    }

    /// Create a range from two positions. The positions are sorted so that
    /// `start <= end`.
    pub fn from_positions(a: Position, b: Position) -> Self {
        if a <= b {
            Self { start: a, end: b }
        } else {
            Self { start: b, end: a }
        }
    }

    /// Returns `true` if the range is empty (start equals end).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns `true` if `pos` is contained within this range.
    ///
    /// A position at the start is inside, a position at the end is **not**.
    pub fn contains_position(&self, pos: &Position) -> bool {
        pos >= &self.start && pos < &self.end
    }

    /// Returns `true` if `other` is fully contained within this range.
    pub fn contains_range(&self, other: &Range) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    /// Returns `true` if this range intersects with `other`.
    pub fn intersects(&self, other: &Range) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Returns the intersection of two ranges, or `None` if they don't
    /// intersect.
    pub fn intersection(&self, other: &Range) -> Option<Range> {
        let start = Position::max(self.start, other.start);
        let end = Position::min(self.end, other.end);
        if start < end {
            Some(Range { start, end })
        } else {
            None
        }
    }

    /// Returns the union/span of two ranges (the smallest range covering
    /// both).
    pub fn plus_range(&self, other: &Range) -> Range {
        Range {
            start: Position::min(self.start, other.start),
            end: Position::max(self.end, other.end),
        }
    }

    /// Collapse this range to its start position.
    pub fn collapse_to_start(&self) -> Range {
        Range {
            start: self.start,
            end: self.start,
        }
    }

    /// Collapse this range to its end position.
    pub fn collapse_to_end(&self) -> Range {
        Range {
            start: self.end,
            end: self.end,
        }
    }

    /// Returns `true` if the range spans a single line.
    pub fn is_single_line(&self) -> bool {
        self.start.line == self.end.line
    }

    /// Returns the number of lines spanned by this range.
    ///
    /// A single-line range returns 1. A range from line 1 to line 3 returns 3.
    pub fn line_count(&self) -> u32 {
        self.end.line - self.start.line + 1
    }

    /// Create a new range by translating both start and end positions.
    pub fn translate(&self, line_delta: i32, column_delta: i32) -> Range {
        Range {
            start: self.start.translate(line_delta, column_delta),
            end: self.end.translate(line_delta, column_delta),
        }
    }

    /// Returns the smallest range that contains both `self` and `other`.
    pub fn union(&self, other: &Range) -> Range {
        Range {
            start: Position::min(self.start, other.start),
            end: Position::max(self.end, other.end),
        }
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Ln {}:Col {} - Ln {}:Col {}",
            self.start.line, self.start.column, self.end.line, self.end.column
        )
    }
}

// ---------------------------------------------------------------------------
// SelectionDirection
// ---------------------------------------------------------------------------

/// Direction of a selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SelectionDirection {
    /// The anchor is before the active position (left-to-right / top-down).
    LeftToRight,
    /// The anchor is after the active position (right-to-left / bottom-up).
    RightToLeft,
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// A selection in the editor, extending `Range` with directional information.
///
/// `anchor` is where the selection was started, `active` is where the cursor
/// currently sits (the "moving" end).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selection {
    /// Where the selection started.
    pub anchor: Position,
    /// Where the cursor currently is (the moving end).
    pub active: Position,
}

impl Selection {
    /// Create a selection from individual coordinates.
    pub fn new(anchor_line: u32, anchor_column: u32, active_line: u32, active_column: u32) -> Self {
        Self {
            anchor: Position::new(anchor_line, anchor_column),
            active: Position::new(active_line, active_column),
        }
    }

    /// Create a selection from two positions.
    pub fn from_positions(anchor: Position, active: Position) -> Self {
        Self { anchor, active }
    }

    /// Create a selection from a range and direction.
    pub fn from_range(range: Range, direction: SelectionDirection) -> Self {
        match direction {
            SelectionDirection::LeftToRight => Self {
                anchor: range.start,
                active: range.end,
            },
            SelectionDirection::RightToLeft => Self {
                anchor: range.end,
                active: range.start,
            },
        }
    }

    /// Returns `true` if the selection is reversed (active is before anchor).
    pub fn is_reversed(&self) -> bool {
        self.active < self.anchor
    }

    /// Convert this selection into a `Range` (always ordered start <= end).
    pub fn as_range(&self) -> Range {
        Range::from_positions(self.anchor, self.active)
    }

    /// Returns the direction of this selection.
    pub fn direction(&self) -> SelectionDirection {
        if self.active < self.anchor {
            SelectionDirection::RightToLeft
        } else {
            SelectionDirection::LeftToRight
        }
    }
}

// ---------------------------------------------------------------------------
// ITextModel trait
// ---------------------------------------------------------------------------

/// Minimal interface for text models consumed by editor components.
pub trait ITextModel {
    /// Returns the total number of lines in the model.
    fn get_line_count(&self) -> u32;

    /// Returns the content of the given 1-based line number.
    fn get_line_content(&self, line_number: u32) -> &str;

    /// Returns the length (in UTF-8 bytes) of the given 1-based line.
    fn get_line_length(&self, line_number: u32) -> u32;

    /// Returns the maximum valid column for the given line (length + 1).
    fn get_line_max_column(&self, line_number: u32) -> u32;

    /// Returns the total length of all text in the model in bytes.
    fn get_value_length(&self) -> usize;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for editor-types operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorTypesStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl EditorTypesStats {
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
    pub fn merge(&mut self, other: &EditorTypesStats) {
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

impl Default for EditorTypesStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EditorTypesStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorTypesStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for editor-types.
#[derive(Debug, Clone)]
pub struct EditorTypesValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl EditorTypesValidator {
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

impl Default for EditorTypesValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TextEdit — a range + replacement text
// ---------------------------------------------------------------------------

/// A single text edit: replace `range` with `new_text`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

impl TextEdit {
    /// Create a new text edit.
    pub fn new(range: Range, new_text: impl Into<String>) -> Self {
        Self { range, new_text: new_text.into() }
    }

    /// Create an insertion edit at a position (empty range).
    pub fn insert(position: Position, text: impl Into<String>) -> Self {
        Self {
            range: Range::from_positions(position, position),
            new_text: text.into(),
        }
    }

    /// Create a deletion edit (replacement text is empty).
    pub fn delete(range: Range) -> Self {
        Self { range, new_text: String::new() }
    }

    /// Whether this edit is a pure insertion (empty range).
    pub fn is_insert(&self) -> bool {
        self.range.is_empty()
    }

    /// Whether this edit is a pure deletion (empty new_text, non-empty range).
    pub fn is_delete(&self) -> bool {
        !self.range.is_empty() && self.new_text.is_empty()
    }

    /// Whether this edit is a replacement (non-empty range and non-empty new_text).
    pub fn is_replace(&self) -> bool {
        !self.range.is_empty() && !self.new_text.is_empty()
    }

    /// Classify this edit as an EditOperation.
    pub fn classify(&self) -> EditOperation {
        if self.is_insert() {
            EditOperation::Insert {
                position: self.range.start,
                text: self.new_text.clone(),
            }
        } else if self.is_delete() {
            EditOperation::Delete { range: self.range }
        } else {
            EditOperation::Replace {
                range: self.range,
                text: self.new_text.clone(),
            }
        }
    }

    /// Apply this edit to lines of text (1-based line numbers).
    /// Returns the modified text as a single string.
    pub fn apply(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut result = String::new();

        // Convert start/end to 0-based indices
        let start_line = (self.range.start.line as usize).saturating_sub(1);
        let start_col = (self.range.start.column as usize).saturating_sub(1);
        let end_line = (self.range.end.line as usize).saturating_sub(1);
        let end_col = (self.range.end.column as usize).saturating_sub(1);

        // Add lines before the edit range
        for (i, line) in lines.iter().enumerate() {
            if i < start_line {
                result.push_str(line);
                result.push('\n');
            } else if i == start_line {
                // Partial first line
                let prefix = &line[..start_col.min(line.len())];
                result.push_str(prefix);
                result.push_str(&self.new_text);

                // If start and end are on the same line
                if start_line == end_line {
                    let suffix_start = end_col.min(line.len());
                    result.push_str(&line[suffix_start..]);
                    result.push('\n');
                }
            } else if i > start_line && i < end_line {
                // Skip lines inside the range
                continue;
            } else if i == end_line && start_line != end_line {
                let suffix_start = end_col.min(line.len());
                result.push_str(&line[suffix_start..]);
                result.push('\n');
            } else if i > end_line {
                result.push_str(line);
                result.push('\n');
            }
        }

        // Handle case where text has no trailing newline
        if !text.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }
        result
    }
}

impl fmt::Display for TextEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_insert() {
            write!(f, "Insert({}, {:?})", self.range.start, self.new_text)
        } else if self.is_delete() {
            write!(f, "Delete({})", self.range)
        } else {
            write!(f, "Replace({}, {:?})", self.range, self.new_text)
        }
    }
}

// ---------------------------------------------------------------------------
// EditOperation enum
// ---------------------------------------------------------------------------

/// Classification of an edit operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditOperation {
    Insert { position: Position, text: String },
    Delete { range: Range },
    Replace { range: Range, text: String },
}

impl EditOperation {
    /// Convert back to a TextEdit.
    pub fn to_text_edit(&self) -> TextEdit {
        match self {
            EditOperation::Insert { position, text } => TextEdit::insert(*position, text.clone()),
            EditOperation::Delete { range } => TextEdit::delete(*range),
            EditOperation::Replace { range, text } => TextEdit::new(*range, text.clone()),
        }
    }

    /// The affected range of this operation.
    pub fn affected_range(&self) -> Range {
        match self {
            EditOperation::Insert { position, .. } => Range::from_positions(*position, *position),
            EditOperation::Delete { range } => *range,
            EditOperation::Replace { range, .. } => *range,
        }
    }
}

impl fmt::Display for EditOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditOperation::Insert { position, text } => {
                write!(f, "Insert at {} ({} chars)", position, text.len())
            }
            EditOperation::Delete { range } => {
                write!(f, "Delete {}", range)
            }
            EditOperation::Replace { range, text } => {
                write!(f, "Replace {} with {} chars", range, text.len())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Batch edit application
// ---------------------------------------------------------------------------

/// Apply multiple text edits to text. Edits are sorted by range in reverse
/// order so later edits don't shift earlier positions.
pub fn apply_edits(text: &str, edits: &[TextEdit]) -> String {
    if edits.is_empty() {
        return text.to_string();
    }
    let mut sorted_edits: Vec<&TextEdit> = edits.iter().collect();
    sorted_edits.sort_by(|a, b| b.range.start.cmp(&a.range.start));

    let mut result = text.to_string();
    for edit in sorted_edits {
        result = edit.apply(&result);
    }
    result
}

// ---------------------------------------------------------------------------
// Range splitting / subtraction
// ---------------------------------------------------------------------------

impl Range {
    /// Subtract `other` from `self`, returning the remaining pieces.
    ///
    /// If `other` does not intersect, returns `[self]`.
    /// If `other` fully contains `self`, returns empty vec.
    /// If `other` partially overlaps, returns 1 or 2 remaining fragments.
    pub fn subtract(&self, other: &Range) -> Vec<Range> {
        if !self.intersects(other) {
            return vec![*self];
        }
        if other.contains_range(self) {
            return Vec::new();
        }
        let mut result = Vec::new();
        // Left fragment: self.start..other.start
        if self.start < other.start {
            result.push(Range { start: self.start, end: other.start });
        }
        // Right fragment: other.end..self.end
        if other.end < self.end {
            result.push(Range { start: other.end, end: self.end });
        }
        result
    }

    /// Split this range at a position, returning (left, right).
    /// If the position is outside the range, one side will be empty.
    pub fn split_at(&self, pos: Position) -> (Range, Range) {
        if pos <= self.start {
            (
                Range { start: self.start, end: self.start },
                *self,
            )
        } else if pos >= self.end {
            (
                *self,
                Range { start: self.end, end: self.end },
            )
        } else {
            (
                Range { start: self.start, end: pos },
                Range { start: pos, end: self.end },
            )
        }
    }

    /// Returns true if this range touches `other` (adjacent or overlapping).
    pub fn touches(&self, other: &Range) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// Merge two touching ranges into one, or return None if disjoint.
    pub fn merge(&self, other: &Range) -> Option<Range> {
        if self.touches(other) {
            Some(Range {
                start: Position::min(self.start, other.start),
                end: Position::max(self.end, other.end),
            })
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Selection direction and position containment
// ---------------------------------------------------------------------------

impl fmt::Display for SelectionDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelectionDirection::LeftToRight => write!(f, "LeftToRight"),
            SelectionDirection::RightToLeft => write!(f, "RightToLeft"),
        }
    }
}

impl Selection {
    /// Returns `true` if this selection is empty (anchor == active).
    pub fn is_empty_selection(&self) -> bool {
        self.anchor == self.active
    }

    /// Returns the length of the selection in lines (inclusive).
    pub fn line_span(&self) -> u32 {
        let range = self.as_range();
        let start = range.start.line;
        let end = range.end.line;
        if start <= end { end - start + 1 } else { start - end + 1 }
    }

    /// Returns the number of lines in this selection.
    ///
    /// Delegates to `as_range().line_count()`.
    pub fn length_in_lines(&self) -> u32 {
        self.as_range().line_count()
    }
}

/// Check whether a position is contained within a range (inclusive of start, exclusive of end).
///
/// A position `p` is "contained" if `range.start <= p < range.end`, following
/// the convention used by VS Code's `Range.contains(Position)`.
pub fn selection_contains_position(range: &Range, pos: &Position) -> bool {
    if pos < &range.start {
        return false;
    }
    if pos >= &range.end {
        return false;
    }
    true
}

/// Check whether a range fully contains another range.
pub fn range_contains_range(outer: &Range, inner: &Range) -> bool {
    selection_contains_position(outer, &inner.start)
        && (inner.end <= outer.end)
}

/// Compute the intersection of two ranges, if any.
pub fn range_intersection(a: &Range, b: &Range) -> Option<Range> {
    let start = Position::max(a.start, b.start);
    let end = Position::min(a.end, b.end);
    if start < end {
        Some(Range::from_positions(start, end))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Extended Position, Range, Selection methods
// ---------------------------------------------------------------------------

impl Position {
    /// Clamp this position so it falls within the bounds of a document
    /// with the given line count and maximum column per line.
    ///
    /// Both `max_line` and `max_column` are inclusive upper bounds.
    pub fn clamp_to_bounds(&self, max_line: u32, max_column: u32) -> Position {
        let line = self.line.clamp(1, max_line.max(1));
        let column = self.column.clamp(1, max_column.max(1));
        Position { line, column }
    }

    /// Compute the Manhattan distance between two positions.
    ///
    /// This is the sum of the absolute differences of their line and column
    /// coordinates: `|a.line - b.line| + |a.column - b.column|`.
    pub fn manhattan_distance(&self, other: &Position) -> u32 {
        self.line.abs_diff(other.line) + self.column.abs_diff(other.column)
    }
}

impl Range {
    /// Shift the entire range by `line_delta` lines and `column_delta` columns
    /// without changing its size.
    ///
    /// This is equivalent to translating both start and end by the same delta,
    /// but differs from `translate` only in name to distinguish intent: shift
    /// preserves shape whereas translate may clamp independently.
    pub fn shift(&self, line_delta: i32, column_delta: i32) -> Range {
        Range {
            start: self.start.translate(line_delta, column_delta),
            end: self.end.translate(line_delta, column_delta),
        }
    }

    /// Returns `true` if this range overlaps (touches) the given 1-based line number.
    pub fn overlaps_line(&self, line: u32) -> bool {
        self.start.line <= line && line <= self.end.line
    }

    /// Decompose a multi-line range into a vector of single-line ranges.
    ///
    /// Each returned range covers one line from column 1 to
    /// `columns_per_line`, except the first and last lines which preserve the
    /// original start/end columns.  `columns_per_line` is used as the end
    /// column for intermediate lines.
    pub fn to_single_line_ranges(&self, columns_per_line: u32) -> Vec<Range> {
        if self.is_empty() {
            return vec![*self];
        }
        let mut result = Vec::new();
        for line in self.start.line..=self.end.line {
            let start_col = if line == self.start.line { self.start.column } else { 1 };
            let end_col = if line == self.end.line { self.end.column } else { columns_per_line };
            result.push(Range {
                start: Position::new(line, start_col),
                end: Position::new(line, end_col),
            });
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Position → offset conversion
// ---------------------------------------------------------------------------

impl Position {
    /// Convert a 1-based position to a 0-based byte offset within the given
    /// line lengths.  `line_lengths` is a slice where index 0 is the length
    /// of line 1 (not counting the newline).  Returns `None` if the position
    /// refers to a line or column that is out of bounds.
    pub fn to_offset(&self, line_lengths: &[u32]) -> Option<usize> {
        if self.line == 0 || self.column == 0 {
            return None;
        }
        let line_idx = (self.line - 1) as usize;
        if line_idx >= line_lengths.len() {
            return None;
        }
        let col_idx = self.column - 1;
        // Allow column == line_length + 1 (one past the end, like a cursor
        // after the last character).
        if col_idx > line_lengths[line_idx] {
            return None;
        }
        let mut offset: usize = 0;
        for &len in &line_lengths[..line_idx] {
            // +1 for the newline character between lines
            offset += len as usize + 1;
        }
        offset += col_idx as usize;
        Some(offset)
    }

    /// Create a position from a 0-based byte offset and a set of line lengths.
    /// Returns `None` if the offset is past the end of the document.
    pub fn from_offset(offset: usize, line_lengths: &[u32]) -> Option<Position> {
        let mut remaining = offset;
        for (i, &len) in line_lengths.iter().enumerate() {
            let line_len_with_nl = len as usize + 1; // +1 for '\n'
            if remaining <= len as usize {
                return Some(Position::new(i as u32 + 1, remaining as u32 + 1));
            }
            // If this is the last line, allow landing right at the end
            if i == line_lengths.len() - 1 {
                if remaining == len as usize {
                    return Some(Position::new(i as u32 + 1, remaining as u32 + 1));
                }
                return None;
            }
            if remaining < line_len_with_nl {
                // offset points to the newline character itself; snap to end of line
                return Some(Position::new(i as u32 + 1, len + 1));
            }
            remaining -= line_len_with_nl;
        }
        None
    }

    /// Returns `true` if this position is on the given 1-based line number.
    pub fn is_on_line(&self, line: u32) -> bool {
        self.line == line
    }

    /// Create a position at the start of the given 1-based line (column 1).
    pub fn line_start(line: u32) -> Position {
        Position::new(line, 1)
    }

    /// Returns a new position with the same line but the given column.
    pub fn with_column(&self, column: u32) -> Position {
        Position::new(self.line, column)
    }

    /// Returns a new position with the same column but the given line.
    pub fn with_line(&self, line: u32) -> Position {
        Position::new(line, self.column)
    }

    /// Returns the delta (line_delta, column_delta) needed to go from `self`
    /// to `other`.
    pub fn delta_to(&self, other: &Position) -> (i64, i64) {
        (
            other.line as i64 - self.line as i64,
            other.column as i64 - self.column as i64,
        )
    }
}

// ---------------------------------------------------------------------------
// Range — advanced operations
// ---------------------------------------------------------------------------

impl Range {
    /// Clamp this range so it falls within the given document bounds.
    pub fn clamp_to_document(&self, max_line: u32, max_column: u32) -> Range {
        Range {
            start: self.start.clamp_to_bounds(max_line, max_column),
            end: self.end.clamp_to_bounds(max_line, max_column),
        }
    }

    /// Expand the range to cover entire lines (column 1 to `max_column`).
    pub fn expand_to_full_lines(&self, max_column: u32) -> Range {
        Range {
            start: Position::new(self.start.line, 1),
            end: Position::new(self.end.line, max_column),
        }
    }

    /// Returns `true` if `pos` is at the start of this range.
    pub fn is_at_start(&self, pos: &Position) -> bool {
        *pos == self.start
    }

    /// Returns `true` if `pos` is at the end of this range.
    pub fn is_at_end(&self, pos: &Position) -> bool {
        *pos == self.end
    }

    /// Returns `true` if the position is contained *or* equals the end
    /// (i.e. inclusive on both ends).
    pub fn contains_position_inclusive(&self, pos: &Position) -> bool {
        pos >= &self.start && pos <= &self.end
    }

    /// Returns the set of 1-based line numbers touched by this range.
    pub fn covered_lines(&self) -> std::ops::RangeInclusive<u32> {
        self.start.line..=self.end.line
    }
}

// ---------------------------------------------------------------------------
// Multi-cursor / multi-selection utilities
// ---------------------------------------------------------------------------

/// Sort a list of ranges by start position, breaking ties by end position.
pub fn sort_ranges(ranges: &mut [Range]) {
    ranges.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
}

/// Merge a sorted slice of ranges, combining any that touch or overlap.
/// Input should be sorted by start position (use `sort_ranges` first).
pub fn merge_sorted_ranges(ranges: &[Range]) -> Vec<Range> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
    merged.push(ranges[0]);
    for r in &ranges[1..] {
        let last = merged.last_mut().unwrap();
        if last.touches(r) {
            *last = last.union(r);
        } else {
            merged.push(*r);
        }
    }
    merged
}

/// Sort and merge a collection of ranges, returning a minimal non-overlapping
/// set of ranges that covers the same area.
pub fn normalize_ranges(ranges: &[Range]) -> Vec<Range> {
    let mut sorted: Vec<Range> = ranges.to_vec();
    sort_ranges(&mut sorted);
    merge_sorted_ranges(&sorted)
}

/// Sort selections by their range start position.
pub fn sort_selections(selections: &mut [Selection]) {
    selections.sort_by(|a, b| {
        let ra = a.as_range();
        let rb = b.as_range();
        ra.start.cmp(&rb.start).then(ra.end.cmp(&rb.end))
    });
}

/// Returns `true` if any of the given selections overlap.
pub fn selections_overlap(selections: &[Selection]) -> bool {
    let mut ranges: Vec<Range> = selections.iter().map(|s| s.as_range()).collect();
    sort_ranges(&mut ranges);
    for i in 1..ranges.len() {
        if ranges[i - 1].intersects(&ranges[i]) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Selection — cursor movement helpers
// ---------------------------------------------------------------------------

impl Selection {
    /// Move the active (cursor) position by the given deltas, preserving the
    /// anchor to extend the selection.
    pub fn extend_active(&self, line_delta: i32, column_delta: i32) -> Selection {
        Selection {
            anchor: self.anchor,
            active: self.active.translate(line_delta, column_delta),
        }
    }

    /// Move both anchor and active by the same deltas (shift without resizing).
    pub fn translate(&self, line_delta: i32, column_delta: i32) -> Selection {
        Selection {
            anchor: self.anchor.translate(line_delta, column_delta),
            active: self.active.translate(line_delta, column_delta),
        }
    }

    /// Create a cursor (zero-width selection) at the given position.
    pub fn cursor(pos: Position) -> Selection {
        Selection {
            anchor: pos,
            active: pos,
        }
    }

    /// Create a cursor at the given 1-based line and column.
    pub fn cursor_at(line: u32, column: u32) -> Selection {
        let pos = Position::new(line, column);
        Selection::cursor(pos)
    }

    /// Returns `true` if the given position is within this selection's range
    /// (inclusive start, exclusive end).
    pub fn contains_position(&self, pos: &Position) -> bool {
        self.as_range().contains_position(pos)
    }
}

impl Selection {
    /// Swap the anchor and active positions, reversing the direction.
    pub fn swap_anchor(&self) -> Selection {
        Selection {
            anchor: self.active,
            active: self.anchor,
        }
    }

    /// Collapse the selection so both anchor and active are at the start
    /// of the current range.
    pub fn collapse_to_start(&self) -> Selection {
        let range = self.as_range();
        Selection {
            anchor: range.start,
            active: range.start,
        }
    }

    /// Collapse the selection so both anchor and active are at the end
    /// of the current range.
    pub fn collapse_to_end(&self) -> Selection {
        let range = self.as_range();
        Selection {
            anchor: range.end,
            active: range.end,
        }
    }

    /// Returns `true` if the selection spans more than one line.
    pub fn is_multi_line(&self) -> bool {
        self.as_range().start.line != self.as_range().end.line
    }

    /// Extend the selection so it covers entire lines.
    pub fn extend_to_full_lines(&self, max_column: u32) -> Selection {
        let range = self.as_range();
        let start = Position::new(range.start.line, 1);
        let end = Position::new(range.end.line, max_column);
        if self.is_reversed() {
            Selection { anchor: end, active: start }
        } else {
            Selection { anchor: start, active: end }
        }
    }
}

// ---------------------------------------------------------------------------
// SelectionComparator - selection comparator
// ---------------------------------------------------------------------------

/// Severity level for selection comparator issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionComparatorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for SelectionComparatorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [SelectionComparator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionComparatorEntry {
    pub id: String,
    pub label: String,
    pub severity: SelectionComparatorSeverity,
    pub detail: Option<String>,
    pub selection_count: usize,
    enabled: bool,
}

impl SelectionComparatorEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: SelectionComparatorSeverity::Low,
            detail: None,
            selection_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: SelectionComparatorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_selection_count(mut self, val: usize) -> Self {
        self.selection_count = val;
        self
    }

    pub fn are_equal(&self) -> bool {
        self.enabled && self.severity >= SelectionComparatorSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.selection_count, det)
    }
}

impl fmt::Display for SelectionComparatorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [SelectionComparatorEntry] items.
#[derive(Debug, Clone)]
pub struct SelectionComparator {
    entries: Vec<SelectionComparatorEntry>,
    name: String,
    capacity: usize,
}

impl SelectionComparator {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: SelectionComparatorEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<SelectionComparatorEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&SelectionComparatorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn selection_count(&self) -> usize { self.entries.len() }

    pub fn are_equal(&self) -> bool {
        self.entries.iter().any(|e| e.are_equal())
    }

    pub fn entries_by_severity(&self, severity: SelectionComparatorSeverity) -> Vec<&SelectionComparatorEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= SelectionComparatorSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&SelectionComparatorEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&SelectionComparatorEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// RangeMerger - range merger utility
// ---------------------------------------------------------------------------

/// Configuration for [RangeMerger].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeMergerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub range_count: usize,
}

impl RangeMergerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, range_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_range_count(mut self, val: usize) -> Self { self.range_count = val; self }
}

impl Default for RangeMergerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [RangeMerger].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeMergerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl RangeMergerItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn can_merge(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for RangeMergerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [RangeMergerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct RangeMerger {
    config: RangeMergerConfig,
    items: Vec<RangeMergerItem>,
}

impl RangeMerger {
    pub fn new(config: RangeMergerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: RangeMergerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<RangeMergerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&RangeMergerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn range_count(&self) -> usize { self.items.len() }

    pub fn can_merge(&self) -> bool {
        self.items.iter().any(|i| i.can_merge())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&RangeMergerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&RangeMergerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &RangeMergerConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-editor-types: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTypesXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl EditorTypesXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for EditorTypesXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct EditorTypesXRegistry {
    entries: Vec<EditorTypesXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl EditorTypesXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: EditorTypesXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&EditorTypesXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut EditorTypesXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<EditorTypesXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&EditorTypesXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&EditorTypesXConfig> {
        let mut sorted: Vec<&EditorTypesXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&EditorTypesXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> EditorTypesXIterator<'_> {
        EditorTypesXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct EditorTypesXIterator<'a> {
    inner: std::slice::Iter<'a, EditorTypesXConfig>,
}

impl<'a> Iterator for EditorTypesXIterator<'a> {
    type Item = &'a EditorTypesXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct EditorTypesXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl EditorTypesXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct EditorTypesXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl EditorTypesXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &EditorTypesXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &EditorTypesXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &EditorTypesXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for EditorTypesXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct EditorTypesXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl EditorTypesXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &EditorTypesXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &EditorTypesXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for EditorTypesXValidator {
    fn default() -> Self {
        Self::new()
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
// xb_ utilities – batch 53
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer53 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer53 {
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
pub fn xb_fnv1a_53(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_53<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_53<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_53(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_53(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 39
// ---------------------------------------------------------------------------

/// Generic object pool `Xc39Pool<T>`.
pub struct Xc39Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc39Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc39PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc39Pool<T> {
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
    pub fn stats(&self) -> Xc39PoolStats {
        Xc39PoolStats {
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

impl<T> Default for Xc39Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc39Scheduler`.
pub struct Xc39Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc39Scheduler {
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

impl Default for Xc39Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_39 hash for the given byte slice.
pub fn xc_39_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_39 convention.
pub fn xc_39_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe66 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe66Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe66PipelineError {
    pub stage: Xe66Stage,
    pub message: String,
}

impl std::fmt::Display for Xe66PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe66Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe66Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError>>>,
    stage_names: Vec<Xe66Stage>,
}

impl Xe66Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe66Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe66Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe66Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe66Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe66Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe66CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe66CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe66Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe66CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe66CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe66Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe66CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_66_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe66CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_66_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe66CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_66_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> {
    Ok(data)
}

pub fn xe_66_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_66_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_66_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_66_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe66PipelineError> {
    Err(Xe66PipelineError {
        stage: Xe66Stage::Parse,
        message: "intentional failure".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Position tests --

    #[test]
    fn position_new() {
        let p = Position::new(3, 5);
        assert_eq!(p.line, 3);
        assert_eq!(p.column, 5);
    }

    #[test]
    fn position_ordering() {
        let a = Position::new(1, 1);
        let b = Position::new(1, 5);
        let c = Position::new(2, 1);

        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
        assert_eq!(a, Position::new(1, 1));
    }

    #[test]
    fn position_is_before() {
        let a = Position::new(1, 3);
        let b = Position::new(1, 5);
        assert!(a.is_before(&b));
        assert!(!b.is_before(&a));
        assert!(!a.is_before(&a));
    }

    #[test]
    fn position_is_before_or_equal() {
        let a = Position::new(1, 3);
        let b = Position::new(1, 5);
        assert!(a.is_before_or_equal(&b));
        assert!(a.is_before_or_equal(&a));
        assert!(!b.is_before_or_equal(&a));
    }

    #[test]
    fn position_is_after() {
        let a = Position::new(2, 1);
        let b = Position::new(1, 5);
        assert!(a.is_after(&b));
        assert!(!b.is_after(&a));
        assert!(!a.is_after(&a));
    }

    #[test]
    fn position_is_after_or_equal() {
        let a = Position::new(2, 1);
        let b = Position::new(1, 5);
        assert!(a.is_after_or_equal(&b));
        assert!(a.is_after_or_equal(&a));
        assert!(!b.is_after_or_equal(&a));
    }

    #[test]
    fn position_equals() {
        let a = Position::new(1, 1);
        let b = Position::new(1, 1);
        let c = Position::new(1, 2);
        assert!(a.equals(&b));
        assert!(!a.equals(&c));
    }

    #[test]
    fn position_min_max() {
        let a = Position::new(1, 3);
        let b = Position::new(2, 1);
        assert_eq!(Position::min(a, b), a);
        assert_eq!(Position::max(a, b), b);
        assert_eq!(Position::min(b, a), a);
        assert_eq!(Position::max(b, a), b);
    }

    #[test]
    fn position_display() {
        assert_eq!(Position::new(1, 5).to_string(), "Ln 1, Col 5");
    }

    #[test]
    fn position_clone_copy() {
        let a = Position::new(1, 1);
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn position_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Position::new(1, 1));
        set.insert(Position::new(1, 1));
        assert_eq!(set.len(), 1);
    }

    // -- Range tests --

    #[test]
    fn range_new() {
        let r = Range::new(1, 1, 2, 5);
        assert_eq!(r.start, Position::new(1, 1));
        assert_eq!(r.end, Position::new(2, 5));
    }

    #[test]
    fn range_from_positions_sorts() {
        let a = Position::new(2, 5);
        let b = Position::new(1, 1);
        let r = Range::from_positions(a, b);
        assert_eq!(r.start, b);
        assert_eq!(r.end, a);
    }

    #[test]
    fn range_is_empty() {
        assert!(Range::new(1, 1, 1, 1).is_empty());
        assert!(!Range::new(1, 1, 1, 2).is_empty());
    }

    #[test]
    fn range_contains_position() {
        let r = Range::new(1, 1, 1, 5);
        assert!(r.contains_position(&Position::new(1, 1)));
        assert!(r.contains_position(&Position::new(1, 3)));
        assert!(!r.contains_position(&Position::new(1, 5))); // end is exclusive
        assert!(!r.contains_position(&Position::new(2, 1)));
    }

    #[test]
    fn range_contains_range() {
        let outer = Range::new(1, 1, 3, 5);
        let inner = Range::new(1, 2, 2, 3);
        let partial = Range::new(2, 1, 4, 1);
        assert!(outer.contains_range(&inner));
        assert!(outer.contains_range(&outer)); // contains itself
        assert!(!outer.contains_range(&partial));
    }

    #[test]
    fn range_intersects() {
        let a = Range::new(1, 1, 2, 5);
        let b = Range::new(2, 3, 3, 1);
        let c = Range::new(3, 1, 4, 1);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn range_intersects_touching_not() {
        let a = Range::new(1, 1, 1, 5);
        let b = Range::new(1, 5, 1, 10);
        // Touching ranges do NOT intersect (end is exclusive).
        assert!(!a.intersects(&b));
    }

    #[test]
    fn range_intersection() {
        let a = Range::new(1, 1, 2, 5);
        let b = Range::new(2, 3, 3, 1);
        let inter = a.intersection(&b).unwrap();
        assert_eq!(inter.start, Position::new(2, 3));
        assert_eq!(inter.end, Position::new(2, 5));
    }

    #[test]
    fn range_intersection_none() {
        let a = Range::new(1, 1, 1, 5);
        let b = Range::new(2, 1, 2, 5);
        assert!(a.intersection(&b).is_none());
    }

    #[test]
    fn range_plus_range() {
        let a = Range::new(1, 3, 2, 1);
        let b = Range::new(1, 1, 3, 5);
        let union = a.plus_range(&b);
        assert_eq!(union.start, Position::new(1, 1));
        assert_eq!(union.end, Position::new(3, 5));
    }

    #[test]
    fn range_collapse_to_start() {
        let r = Range::new(1, 3, 2, 5);
        let collapsed = r.collapse_to_start();
        assert!(collapsed.is_empty());
        assert_eq!(collapsed.start, Position::new(1, 3));
    }

    #[test]
    fn range_collapse_to_end() {
        let r = Range::new(1, 3, 2, 5);
        let collapsed = r.collapse_to_end();
        assert!(collapsed.is_empty());
        assert_eq!(collapsed.start, Position::new(2, 5));
    }

    #[test]
    fn range_is_single_line() {
        assert!(Range::new(1, 1, 1, 5).is_single_line());
        assert!(!Range::new(1, 1, 2, 1).is_single_line());
    }

    #[test]
    fn range_display() {
        assert_eq!(
            Range::new(1, 1, 2, 5).to_string(),
            "Ln 1:Col 1 - Ln 2:Col 5"
        );
    }

    // -- Selection tests --

    #[test]
    fn selection_new() {
        let s = Selection::new(1, 1, 2, 5);
        assert_eq!(s.anchor, Position::new(1, 1));
        assert_eq!(s.active, Position::new(2, 5));
    }

    #[test]
    fn selection_from_positions() {
        let anchor = Position::new(1, 1);
        let active = Position::new(2, 5);
        let s = Selection::from_positions(anchor, active);
        assert_eq!(s.anchor, anchor);
        assert_eq!(s.active, active);
    }

    #[test]
    fn selection_from_range_ltr() {
        let r = Range::new(1, 1, 2, 5);
        let s = Selection::from_range(r, SelectionDirection::LeftToRight);
        assert_eq!(s.anchor, r.start);
        assert_eq!(s.active, r.end);
        assert!(!s.is_reversed());
    }

    #[test]
    fn selection_from_range_rtl() {
        let r = Range::new(1, 1, 2, 5);
        let s = Selection::from_range(r, SelectionDirection::RightToLeft);
        assert_eq!(s.anchor, r.end);
        assert_eq!(s.active, r.start);
        assert!(s.is_reversed());
    }

    #[test]
    fn selection_is_reversed() {
        let forward = Selection::new(1, 1, 2, 5);
        let backward = Selection::new(2, 5, 1, 1);
        assert!(!forward.is_reversed());
        assert!(backward.is_reversed());
    }

    #[test]
    fn selection_as_range() {
        let s = Selection::new(2, 5, 1, 1);
        let r = s.as_range();
        assert_eq!(r.start, Position::new(1, 1));
        assert_eq!(r.end, Position::new(2, 5));
    }

    #[test]
    fn selection_direction() {
        let forward = Selection::new(1, 1, 2, 5);
        let backward = Selection::new(2, 5, 1, 1);
        let collapsed = Selection::new(1, 1, 1, 1);
        assert_eq!(forward.direction(), SelectionDirection::LeftToRight);
        assert_eq!(backward.direction(), SelectionDirection::RightToLeft);
        assert_eq!(collapsed.direction(), SelectionDirection::LeftToRight);
    }

    #[test]
    fn selection_clone_copy() {
        let a = Selection::new(1, 1, 2, 5);
        let b = a;
        assert_eq!(a, b);
    }

    // -- SelectionDirection tests --

    #[test]
    fn selection_direction_eq() {
        assert_eq!(
            SelectionDirection::LeftToRight,
            SelectionDirection::LeftToRight
        );
        assert_ne!(
            SelectionDirection::LeftToRight,
            SelectionDirection::RightToLeft
        );
    }

    // -- Serde round-trip tests --

    #[test]
    fn position_serde_roundtrip() {
        let p = Position::new(10, 20);
        let json = serde_json::to_string(&p).unwrap();
        let p2: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(p, p2);
    }

    #[test]
    fn range_serde_roundtrip() {
        let r = Range::new(1, 1, 5, 10);
        let json = serde_json::to_string(&r).unwrap();
        let r2: Range = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    #[test]
    fn selection_serde_roundtrip() {
        let s = Selection::new(1, 1, 5, 10);
        let json = serde_json::to_string(&s).unwrap();
        let s2: Selection = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }

    // -- ITextModel tests --

    struct SimpleModel {
        lines: Vec<String>,
    }

    impl ITextModel for SimpleModel {
        fn get_line_count(&self) -> u32 {
            self.lines.len() as u32
        }

        fn get_line_content(&self, line_number: u32) -> &str {
            &self.lines[(line_number - 1) as usize]
        }

        fn get_line_length(&self, line_number: u32) -> u32 {
            self.lines[(line_number - 1) as usize].len() as u32
        }

        fn get_line_max_column(&self, line_number: u32) -> u32 {
            self.get_line_length(line_number) + 1
        }

        fn get_value_length(&self) -> usize {
            let content_len: usize = self.lines.iter().map(|l| l.len()).sum();
            let newlines = if self.lines.len() > 1 {
                self.lines.len() - 1
            } else {
                0
            };
            content_len + newlines
        }
    }

    #[test]
    fn text_model_basic() {
        let model = SimpleModel {
            lines: vec!["hello".into(), "world".into(), "!".into()],
        };
        assert_eq!(model.get_line_count(), 3);
        assert_eq!(model.get_line_content(1), "hello");
        assert_eq!(model.get_line_content(2), "world");
        assert_eq!(model.get_line_length(1), 5);
        assert_eq!(model.get_line_max_column(1), 6);
        assert_eq!(model.get_value_length(), 13); // "hello\nworld\n!"
    }

    #[test]
    fn text_model_single_line() {
        let model = SimpleModel {
            lines: vec!["abc".into()],
        };
        assert_eq!(model.get_line_count(), 1);
        assert_eq!(model.get_line_length(1), 3);
        assert_eq!(model.get_line_max_column(1), 4);
        assert_eq!(model.get_value_length(), 3);
    }

    #[test]
    fn editor_types_stats_new_defaults() {
        let stats = EditorTypesStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn editor_types_stats_record_success() {
        let mut stats = EditorTypesStats::new();
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
    fn editor_types_stats_record_failure() {
        let mut stats = EditorTypesStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn editor_types_stats_reset() {
        let mut stats = EditorTypesStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn editor_types_stats_merge() {
        let mut a = EditorTypesStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = EditorTypesStats::new();
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
    fn editor_types_stats_display() {
        let mut stats = EditorTypesStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn editor_types_stats_default() {
        let stats = EditorTypesStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn editor_types_validator_accepts_valid_name() {
        let v = EditorTypesValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn editor_types_validator_rejects_empty() {
        let v = EditorTypesValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn editor_types_validator_rejects_too_long() {
        let v = EditorTypesValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn editor_types_validator_forbidden_prefix() {
        let v = EditorTypesValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn editor_types_validator_allowed_chars() {
        let v = EditorTypesValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn editor_types_validator_range() {
        let v = EditorTypesValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn editor_types_sanitize_removes_control() {
        let result = EditorTypesValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn editor_types_truncate_short_string() {
        assert_eq!(EditorTypesValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn editor_types_truncate_long_string() {
        let result = EditorTypesValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn editor_types_is_ascii_printable() {
        assert!(EditorTypesValidator::is_ascii_printable("Hello World 123"));
        assert!(!EditorTypesValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn selection_direction_forward() {
        let sel = Selection::from_positions(Position::new(1, 1), Position::new(1, 10));
        assert_eq!(sel.direction(), SelectionDirection::LeftToRight);
    }

    #[test]
    fn selection_direction_backward() {
        let sel = Selection::from_positions(Position::new(1, 10), Position::new(1, 1));
        assert_eq!(sel.direction(), SelectionDirection::RightToLeft);
    }

    #[test]
    fn selection_direction_empty() {
        let sel = Selection::from_positions(Position::new(3, 5), Position::new(3, 5));
        assert_eq!(sel.direction(), SelectionDirection::LeftToRight);
        assert!(sel.is_empty_selection());
    }

    #[test]
    fn selection_contains_position_inside() {
        let range = Range::new(1, 1, 1, 10);
        assert!(selection_contains_position(&range, &Position::new(1, 5)));
        assert!(selection_contains_position(&range, &Position::new(1, 1)));
        assert!(!selection_contains_position(&range, &Position::new(1, 10))); // exclusive end
    }

    #[test]
    fn selection_contains_position_outside() {
        let range = Range::new(2, 1, 5, 1);
        assert!(!selection_contains_position(&range, &Position::new(1, 1)));
        assert!(!selection_contains_position(&range, &Position::new(6, 1)));
    }

    #[test]
    fn range_intersection_overlapping() {
        let a = Range::new(1, 1, 3, 1);
        let b = Range::new(2, 1, 5, 1);
        let inter = super::range_intersection(&a, &b).unwrap();
        assert_eq!(inter.start, Position::new(2, 1));
        assert_eq!(inter.end, Position::new(3, 1));
    }

    #[test]
    fn range_intersection_no_overlap() {
        let a = Range::new(1, 1, 2, 1);
        let b = Range::new(3, 1, 4, 1);
        assert!(super::range_intersection(&a, &b).is_none());
    }

    #[test]
    fn selection_line_span() {
        let sel = Selection::from_positions(Position::new(2, 1), Position::new(5, 10));
        assert_eq!(sel.line_span(), 4);
    }

    // -- New functionality tests --

    #[test]
    fn position_origin() {
        let p = Position::origin();
        assert_eq!(p.line, 0);
        assert_eq!(p.column, 0);
    }

    #[test]
    fn position_translate_positive() {
        let p = Position::new(3, 5);
        let t = p.translate(2, 3);
        assert_eq!(t, Position::new(5, 8));
    }

    #[test]
    fn position_translate_negative_clamped() {
        let p = Position::new(1, 2);
        let t = p.translate(-10, -10);
        assert_eq!(t, Position::new(0, 0));
    }

    #[test]
    fn range_line_count_single_line() {
        let r = Range::new(3, 1, 3, 10);
        assert_eq!(r.line_count(), 1);
    }

    #[test]
    fn range_line_count_multi_line() {
        let r = Range::new(2, 1, 7, 5);
        assert_eq!(r.line_count(), 6);
    }

    #[test]
    fn range_translate() {
        let r = Range::new(1, 1, 3, 5);
        let t = r.translate(10, 2);
        assert_eq!(t.start, Position::new(11, 3));
        assert_eq!(t.end, Position::new(13, 7));
    }

    #[test]
    fn range_union() {
        let a = Range::new(3, 5, 5, 1);
        let b = Range::new(1, 1, 4, 3);
        let u = a.union(&b);
        assert_eq!(u.start, Position::new(1, 1));
        assert_eq!(u.end, Position::new(5, 1));
    }

    #[test]
    fn selection_length_in_lines() {
        let sel = Selection::new(5, 1, 2, 3);
        assert_eq!(sel.length_in_lines(), 4);
    }

    #[test]
    fn position_translate_zero_delta() {
        let p = Position::new(4, 7);
        assert_eq!(p.translate(0, 0), p);
    }

    #[test]
    fn range_union_disjoint() {
        let a = Range::new(1, 1, 2, 1);
        let b = Range::new(5, 1, 6, 1);
        let u = a.union(&b);
        assert_eq!(u.start, Position::new(1, 1));
        assert_eq!(u.end, Position::new(6, 1));
    }

    // ---- TextEdit tests ----

    #[test]
    fn text_edit_classify_insert() {
        let edit = TextEdit::insert(Position::new(1, 1), "hello");
        assert!(edit.is_insert());
        assert!(!edit.is_delete());
        assert!(!edit.is_replace());
        match edit.classify() {
            EditOperation::Insert { position, text } => {
                assert_eq!(position, Position::new(1, 1));
                assert_eq!(text, "hello");
            }
            _ => panic!("Expected Insert"),
        }
    }

    #[test]
    fn text_edit_classify_delete() {
        let edit = TextEdit::delete(Range::new(1, 1, 1, 5));
        assert!(edit.is_delete());
        assert!(!edit.is_insert());
    }

    #[test]
    fn text_edit_apply_single_line_replace() {
        let text = "hello world";
        let edit = TextEdit::new(Range::new(1, 1, 1, 6), "goodbye");
        let result = edit.apply(text);
        assert_eq!(result, "goodbye world");
    }

    #[test]
    fn edit_operation_roundtrip() {
        let edit = TextEdit::new(Range::new(1, 1, 2, 3), "replacement");
        let op = edit.classify();
        let back = op.to_text_edit();
        assert_eq!(back.range, edit.range);
        assert_eq!(back.new_text, edit.new_text);
    }

    // ---- Range subtraction / splitting tests ----

    #[test]
    fn range_subtract_no_overlap() {
        let a = Range::new(1, 1, 2, 1);
        let b = Range::new(5, 1, 6, 1);
        let result = a.subtract(&b);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], a);
    }

    #[test]
    fn range_subtract_full_cover() {
        let a = Range::new(2, 1, 3, 1);
        let b = Range::new(1, 1, 5, 1);
        let result = a.subtract(&b);
        assert!(result.is_empty());
    }

    #[test]
    fn range_split_at_middle() {
        let r = Range::new(1, 1, 3, 1);
        let (left, right) = r.split_at(Position::new(2, 1));
        assert_eq!(left.start, Position::new(1, 1));
        assert_eq!(left.end, Position::new(2, 1));
        assert_eq!(right.start, Position::new(2, 1));
        assert_eq!(right.end, Position::new(3, 1));
    }

    #[test]
    fn range_merge_adjacent() {
        let a = Range::new(1, 1, 2, 1);
        let b = Range::new(2, 1, 3, 1);
        let merged = a.merge(&b);
        assert!(merged.is_some());
        let m = merged.unwrap();
        assert_eq!(m.start, Position::new(1, 1));
        assert_eq!(m.end, Position::new(3, 1));
    }

    #[test]
    fn range_touches_disjoint() {
        let a = Range::new(1, 1, 2, 1);
        let b = Range::new(3, 1, 4, 1);
        assert!(!a.touches(&b));
        assert!(a.merge(&b).is_none());
    }

    // -- New functionality tests --

    #[test]
    fn position_clamp_within_bounds() {
        let p = Position::new(5, 10);
        let clamped = p.clamp_to_bounds(100, 80);
        assert_eq!(clamped, Position::new(5, 10));
    }

    #[test]
    fn position_clamp_exceeds_bounds() {
        let p = Position::new(200, 50);
        let clamped = p.clamp_to_bounds(100, 20);
        assert_eq!(clamped, Position::new(100, 20));
    }

    #[test]
    fn position_manhattan_distance_same() {
        let a = Position::new(3, 7);
        assert_eq!(a.manhattan_distance(&a), 0);
    }

    #[test]
    fn position_manhattan_distance_different() {
        let a = Position::new(1, 1);
        let b = Position::new(4, 6);
        assert_eq!(a.manhattan_distance(&b), 8); // 3 + 5
    }

    #[test]
    fn range_shift_positive() {
        let r = Range::new(1, 1, 1, 10);
        let shifted = r.shift(5, 3);
        assert_eq!(shifted.start, Position::new(6, 4));
        assert_eq!(shifted.end, Position::new(6, 13));
    }

    #[test]
    fn range_overlaps_line_true() {
        let r = Range::new(2, 1, 5, 10);
        assert!(r.overlaps_line(3));
        assert!(r.overlaps_line(2));
        assert!(r.overlaps_line(5));
    }

    #[test]
    fn range_overlaps_line_false() {
        let r = Range::new(2, 1, 5, 10);
        assert!(!r.overlaps_line(1));
        assert!(!r.overlaps_line(6));
    }

    #[test]
    fn range_to_single_line_ranges_single() {
        let r = Range::new(3, 5, 3, 15);
        let lines = r.to_single_line_ranges(80);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], r);
    }

    #[test]
    fn range_to_single_line_ranges_multi() {
        let r = Range::new(1, 5, 3, 10);
        let lines = r.to_single_line_ranges(80);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].start, Position::new(1, 5));
        assert_eq!(lines[0].end, Position::new(1, 80));
        assert_eq!(lines[1].start, Position::new(2, 1));
        assert_eq!(lines[1].end, Position::new(2, 80));
        assert_eq!(lines[2].start, Position::new(3, 1));
        assert_eq!(lines[2].end, Position::new(3, 10));
    }

    #[test]
    fn selection_swap_anchor() {
        let sel = Selection::new(1, 1, 3, 5);
        let swapped = sel.swap_anchor();
        assert_eq!(swapped.anchor, Position::new(3, 5));
        assert_eq!(swapped.active, Position::new(1, 1));
    }

    #[test]
    fn selection_collapse_to_start() {
        let sel = Selection::new(5, 10, 1, 1);
        let collapsed = sel.collapse_to_start();
        assert_eq!(collapsed.anchor, Position::new(1, 1));
        assert_eq!(collapsed.active, Position::new(1, 1));
        assert!(collapsed.is_empty_selection());
    }

    #[test]
    fn selection_collapse_to_end() {
        let sel = Selection::new(1, 1, 5, 10);
        let collapsed = sel.collapse_to_end();
        assert_eq!(collapsed.anchor, Position::new(5, 10));
        assert_eq!(collapsed.active, Position::new(5, 10));
        assert!(collapsed.is_empty_selection());
    }

    #[test]
    fn selection_is_multi_line() {
        let single = Selection::new(1, 1, 1, 10);
        assert!(!single.is_multi_line());
        let multi = Selection::new(1, 1, 3, 5);
        assert!(multi.is_multi_line());
    }

    #[test]
    fn selection_extend_to_full_lines() {
        let sel = Selection::new(2, 5, 4, 10);
        let extended = sel.extend_to_full_lines(80);
        assert_eq!(extended.anchor, Position::new(2, 1));
        assert_eq!(extended.active, Position::new(4, 80));
    }

    // -- Position offset conversion tests --

    #[test]
    fn position_to_offset_basic() {
        // "hello\nworld\n!" → line_lengths = [5, 5, 1]
        let lens = [5, 5, 1];
        assert_eq!(Position::new(1, 1).to_offset(&lens), Some(0));
        assert_eq!(Position::new(1, 6).to_offset(&lens), Some(5));
        assert_eq!(Position::new(2, 1).to_offset(&lens), Some(6));
        assert_eq!(Position::new(2, 3).to_offset(&lens), Some(8));
        assert_eq!(Position::new(3, 1).to_offset(&lens), Some(12));
        assert_eq!(Position::new(3, 2).to_offset(&lens), Some(13));
    }

    #[test]
    fn position_to_offset_out_of_bounds() {
        let lens = [5, 5];
        assert_eq!(Position::new(0, 1).to_offset(&lens), None);
        assert_eq!(Position::new(1, 0).to_offset(&lens), None);
        assert_eq!(Position::new(3, 1).to_offset(&lens), None);
        assert_eq!(Position::new(1, 7).to_offset(&lens), None); // col > len+1
    }

    #[test]
    fn position_from_offset_basic() {
        let lens = [5, 5, 1];
        assert_eq!(Position::from_offset(0, &lens), Some(Position::new(1, 1)));
        assert_eq!(Position::from_offset(5, &lens), Some(Position::new(1, 6)));
        assert_eq!(Position::from_offset(6, &lens), Some(Position::new(2, 1)));
        assert_eq!(Position::from_offset(12, &lens), Some(Position::new(3, 1)));
    }

    #[test]
    fn position_from_offset_past_end() {
        let lens = [3];
        assert_eq!(Position::from_offset(3, &lens), Some(Position::new(1, 4)));
        assert_eq!(Position::from_offset(4, &lens), None);
    }

    #[test]
    fn position_delta_to() {
        let a = Position::new(1, 5);
        let b = Position::new(3, 2);
        assert_eq!(a.delta_to(&b), (2, -3));
        assert_eq!(b.delta_to(&a), (-2, 3));
    }

    #[test]
    fn position_with_column_and_line() {
        let p = Position::new(3, 7);
        assert_eq!(p.with_column(1), Position::new(3, 1));
        assert_eq!(p.with_line(10), Position::new(10, 7));
    }

    #[test]
    fn position_line_start() {
        assert_eq!(Position::line_start(5), Position::new(5, 1));
    }

    #[test]
    fn position_is_on_line() {
        let p = Position::new(4, 10);
        assert!(p.is_on_line(4));
        assert!(!p.is_on_line(5));
    }

    // -- Range advanced tests --

    #[test]
    fn range_expand_to_full_lines() {
        let r = Range::new(2, 5, 4, 10);
        let expanded = r.expand_to_full_lines(80);
        assert_eq!(expanded.start, Position::new(2, 1));
        assert_eq!(expanded.end, Position::new(4, 80));
    }

    #[test]
    fn range_clamp_to_document() {
        let r = Range::new(0, 0, 200, 300);
        let clamped = r.clamp_to_document(100, 80);
        assert_eq!(clamped.start, Position::new(1, 1));
        assert_eq!(clamped.end, Position::new(100, 80));
    }

    #[test]
    fn range_is_at_start_and_end() {
        let r = Range::new(2, 3, 5, 10);
        assert!(r.is_at_start(&Position::new(2, 3)));
        assert!(!r.is_at_start(&Position::new(2, 4)));
        assert!(r.is_at_end(&Position::new(5, 10)));
        assert!(!r.is_at_end(&Position::new(5, 9)));
    }

    #[test]
    fn range_contains_position_inclusive() {
        let r = Range::new(1, 1, 1, 5);
        assert!(r.contains_position_inclusive(&Position::new(1, 1)));
        assert!(r.contains_position_inclusive(&Position::new(1, 5))); // inclusive!
        assert!(!r.contains_position_inclusive(&Position::new(1, 6)));
    }

    #[test]
    fn range_covered_lines() {
        let r = Range::new(3, 1, 7, 10);
        let lines: Vec<u32> = r.covered_lines().collect();
        assert_eq!(lines, vec![3, 4, 5, 6, 7]);
    }

    // -- Multi-cursor / normalize tests --

    #[test]
    fn sort_and_merge_ranges() {
        let ranges = vec![
            Range::new(5, 1, 6, 1),
            Range::new(1, 1, 3, 1),
            Range::new(2, 5, 5, 5),
        ];
        let normalized = normalize_ranges(&ranges);
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].start, Position::new(1, 1));
        assert_eq!(normalized[0].end, Position::new(6, 1));
    }

    #[test]
    fn normalize_ranges_disjoint() {
        let ranges = vec![
            Range::new(1, 1, 2, 1),
            Range::new(5, 1, 6, 1),
        ];
        let normalized = normalize_ranges(&ranges);
        assert_eq!(normalized.len(), 2);
    }

    #[test]
    fn selections_overlap_true() {
        let sels = vec![
            Selection::new(1, 1, 3, 1),
            Selection::new(2, 1, 4, 1),
        ];
        assert!(selections_overlap(&sels));
    }

    #[test]
    fn selections_overlap_false() {
        let sels = vec![
            Selection::new(1, 1, 2, 1),
            Selection::new(3, 1, 4, 1),
        ];
        assert!(!selections_overlap(&sels));
    }

    #[test]
    fn sort_selections_by_range() {
        let mut sels = vec![
            Selection::new(5, 1, 6, 1),
            Selection::new(1, 1, 2, 1),
            Selection::new(3, 1, 4, 1),
        ];
        sort_selections(&mut sels);
        assert_eq!(sels[0].anchor, Position::new(1, 1));
        assert_eq!(sels[1].anchor, Position::new(3, 1));
        assert_eq!(sels[2].anchor, Position::new(5, 1));
    }

    // -- Selection cursor / movement tests --

    #[test]
    fn selection_cursor_creates_empty() {
        let sel = Selection::cursor(Position::new(3, 7));
        assert!(sel.is_empty_selection());
        assert_eq!(sel.anchor, Position::new(3, 7));
        assert_eq!(sel.active, Position::new(3, 7));
    }

    #[test]
    fn selection_cursor_at() {
        let sel = Selection::cursor_at(5, 10);
        assert!(sel.is_empty_selection());
        assert_eq!(sel.anchor, Position::new(5, 10));
    }

    #[test]
    fn selection_extend_active_grows() {
        let sel = Selection::new(1, 1, 1, 5);
        let extended = sel.extend_active(2, 3);
        assert_eq!(extended.anchor, Position::new(1, 1));
        assert_eq!(extended.active, Position::new(3, 8));
    }

    #[test]
    fn selection_translate_moves_both() {
        let sel = Selection::new(2, 3, 4, 7);
        let moved = sel.translate(1, 2);
        assert_eq!(moved.anchor, Position::new(3, 5));
        assert_eq!(moved.active, Position::new(5, 9));
    }

    #[test]
    fn selection_contains_position_method() {
        let sel = Selection::new(1, 1, 1, 10);
        assert!(sel.contains_position(&Position::new(1, 5)));
        assert!(!sel.contains_position(&Position::new(1, 10))); // exclusive end
    }

#[test]
    fn selectioncomparator_severity_ordering() {
        assert!(SelectionComparatorSeverity::Critical > SelectionComparatorSeverity::High);
        assert!(SelectionComparatorSeverity::High > SelectionComparatorSeverity::Medium);
        assert!(SelectionComparatorSeverity::Medium > SelectionComparatorSeverity::Low);
    }

    #[test]
    fn selectioncomparator_severity_display() {
        assert_eq!(SelectionComparatorSeverity::Low.to_string(), "low");
        assert_eq!(SelectionComparatorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn selectioncomparator_entry_creation() {
        let e = SelectionComparatorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, SelectionComparatorSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn selectioncomparator_entry_builder() {
        let e = SelectionComparatorEntry::new("e2", "Entry 2")
            .with_severity(SelectionComparatorSeverity::High)
            .with_detail("some detail")
            .with_selection_count(42);
        assert_eq!(e.severity, SelectionComparatorSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.selection_count, 42);
    }

    #[test]
    fn selectioncomparator_entry_enable_disable() {
        let mut e = SelectionComparatorEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn selectioncomparator_add_and_count() {
        let mut mgr = SelectionComparator::new("test");
        mgr.add(SelectionComparatorEntry::new("a", "A"));
        mgr.add(SelectionComparatorEntry::new("b", "B").with_severity(SelectionComparatorSeverity::High));
        assert_eq!(mgr.selection_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn selectioncomparator_remove() {
        let mut mgr = SelectionComparator::new("test");
        mgr.add(SelectionComparatorEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn selectioncomparator_capacity() {
        let mut mgr = SelectionComparator::new("test").with_capacity(1);
        assert!(mgr.add(SelectionComparatorEntry::new("a", "A")));
        assert!(!mgr.add(SelectionComparatorEntry::new("b", "B")));
    }

    #[test]
    fn selectioncomparator_sorted_by_severity() {
        let mut mgr = SelectionComparator::new("test");
        mgr.add(SelectionComparatorEntry::new("lo", "Low"));
        mgr.add(SelectionComparatorEntry::new("hi", "High").with_severity(SelectionComparatorSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, SelectionComparatorSeverity::Critical);
    }

    #[test]
    fn selectioncomparator_summary() {
        let mgr = SelectionComparator::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn rangemerger_config_defaults() {
        let cfg = RangeMergerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn rangemerger_item_creation() {
        let item = RangeMergerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn rangemerger_add_and_get() {
        let mut mgr = RangeMerger::new(RangeMergerConfig::new("test"));
        mgr.add(RangeMergerItem::new("k1", "v1"));
        assert_eq!(mgr.range_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn rangemerger_remove_item() {
        let mut mgr = RangeMerger::new(RangeMergerConfig::new("test"));
        mgr.add(RangeMergerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn rangemerger_sorted_by_priority() {
        let mut mgr = RangeMerger::new(RangeMergerConfig::new("test"));
        mgr.add(RangeMergerItem::new("lo", "low").with_priority(1));
        mgr.add(RangeMergerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn rangemerger_items_with_tag() {
        let mut mgr = RangeMerger::new(RangeMergerConfig::new("test"));
        mgr.add(RangeMergerItem::new("a", "1").with_tag("x"));
        mgr.add(RangeMergerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn rangemerger_report() {
        let mgr = RangeMerger::new(RangeMergerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn editorTypes_x_config_new() {
        let c = EditorTypesXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn editorTypes_x_config_builder() {
        let c = EditorTypesXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn editorTypes_x_config_display() {
        let c = EditorTypesXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn editorTypes_x_registry_insert_get() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn editorTypes_x_registry_duplicate() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a")).unwrap();
        assert!(reg.insert(EditorTypesXConfig::new("a")).is_err());
    }

    #[test]
    fn editorTypes_x_registry_remove() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a")).unwrap();
        reg.insert(EditorTypesXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn editorTypes_x_registry_active_entries() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a")).unwrap();
        reg.insert(EditorTypesXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn editorTypes_x_registry_by_weight() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(EditorTypesXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn editorTypes_x_registry_tags() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(EditorTypesXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn editorTypes_x_registry_total_weight() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(EditorTypesXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn editorTypes_x_registry_iterator() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a")).unwrap();
        reg.insert(EditorTypesXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn editorTypes_x_cache_put_get() {
        let mut cache = EditorTypesXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn editorTypes_x_cache_eviction() {
        let mut cache = EditorTypesXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn editorTypes_x_cache_lru_order() {
        let mut cache = EditorTypesXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn editorTypes_x_cache_most_least_recent() {
        let mut cache = EditorTypesXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn editorTypes_x_formatter_entry() {
        let e = EditorTypesXConfig::new("k").with_value("v");
        let fmt = EditorTypesXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn editorTypes_x_formatter_summary() {
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("a").with_weight(5)).unwrap();
        let fmt = EditorTypesXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn editorTypes_x_validator_valid() {
        let v = EditorTypesXValidator::new();
        let c = EditorTypesXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn editorTypes_x_validator_empty_key() {
        let v = EditorTypesXValidator::new();
        let c = EditorTypesXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn editorTypes_x_validator_require_value() {
        let v = EditorTypesXValidator::new().require_value(true);
        let c = EditorTypesXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn editorTypes_x_validator_allowed_tags() {
        let v = EditorTypesXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = EditorTypesXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn editorTypes_x_validator_validate_all() {
        let v = EditorTypesXValidator::new();
        let mut reg = EditorTypesXRegistry::new();
        reg.insert(EditorTypesXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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
    fn xb_ring_buffer_53_push_and_len() {
        let mut rb = super::XbRingBuffer53::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_53_overwrite() {
        let mut rb = super::XbRingBuffer53::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_53_get_out_of_bounds() {
        let rb = super::XbRingBuffer53::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_53_drain_all() {
        let mut rb = super::XbRingBuffer53::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_53_peek_front_back() {
        let mut rb = super::XbRingBuffer53::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_53_clear() {
        let mut rb = super::XbRingBuffer53::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_53_capacity() {
        let rb = super::XbRingBuffer53::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_53_basic() {
        let h = super::xb_fnv1a_53(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_53(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_53_different_inputs() {
        let h1 = super::xb_fnv1a_53(b"abc");
        let h2 = super::xb_fnv1a_53(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_53_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_53(&data);
        let dec = super::xb_rle_decode_53(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_53_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_53(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_53(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_53_values() {
        assert!((super::xb_clamp_53(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_53(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_53(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_53_values() {
        assert!((super::xb_lerp_53(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_53(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_53(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_53_wrap_around_twice() {
        let mut rb = super::XbRingBuffer53::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 39 ----

    #[test]
    fn xc_39_pool_new_empty() {
        let pool: super::Xc39Pool<i32> = super::Xc39Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_39_pool_release_acquire() {
        let mut pool = super::Xc39Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_39_pool_acquire_empty() {
        let mut pool: super::Xc39Pool<i32> = super::Xc39Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_39_pool_full() {
        let mut pool = super::Xc39Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_39_pool_drain() {
        let mut pool = super::Xc39Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_39_pool_stats() {
        let mut pool = super::Xc39Pool::new(8);
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
    fn xc_39_pool_clear() {
        let mut pool = super::Xc39Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_39_pool_shrink() {
        let mut pool = super::Xc39Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_39_pool_default() {
        let pool: super::Xc39Pool<String> = super::Xc39Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_39_pool_extend() {
        let mut pool = super::Xc39Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_39_pool_retain() {
        let mut pool = super::Xc39Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_39_scheduler_round_robin() {
        let mut sched = super::Xc39Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_39_scheduler_empty() {
        let mut sched = super::Xc39Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_39_scheduler_reset() {
        let mut sched = super::Xc39Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_39_scheduler_add_remove() {
        let mut sched = super::Xc39Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_39_scheduler_targets() {
        let sched = super::Xc39Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_39_hash_empty() {
        assert_eq!(super::xc_39_hash(b""), 5381);
    }

    #[test]
    fn xc_39_hash_data() {
        let h = super::xc_39_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_39_hash(b"hello"), h);
    }

    #[test]
    fn xc_39_reverse_str() {
        assert_eq!(super::xc_39_reverse("abc"), "cba");
        assert_eq!(super::xc_39_reverse(""), "");
    }


    #[test]
    fn xe_66_pipeline_empty() {
        let p = super::Xe66Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_66_pipeline_parse_stage() {
        let p = super::Xe66Pipeline::new()
            .add_parse(super::xe_66_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_66_pipeline_transform_double() {
        let p = super::Xe66Pipeline::new()
            .add_transform(super::xe_66_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_66_pipeline_validate_reverse() {
        let p = super::Xe66Pipeline::new()
            .add_validate(super::xe_66_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_66_pipeline_emit_filter() {
        let p = super::Xe66Pipeline::new()
            .add_emit(super::xe_66_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_66_pipeline_multi_stage() {
        let p = super::Xe66Pipeline::new()
            .add_parse(super::xe_66_pipeline_identity)
            .add_transform(super::xe_66_pipeline_double)
            .add_validate(super::xe_66_pipeline_reverse)
            .add_emit(super::xe_66_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_66_pipeline_error_propagation() {
        let p = super::Xe66Pipeline::new()
            .add_parse(super::xe_66_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe66Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_66_pipeline_compose() {
        let p1 = super::Xe66Pipeline::new()
            .add_parse(super::xe_66_pipeline_identity);
        let p2 = super::Xe66Pipeline::new()
            .add_transform(super::xe_66_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_66_pipeline_error_display() {
        let e = super::Xe66PipelineError {
            stage: super::Xe66Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_66_cache_put_get() {
        let mut c = super::Xe66Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_66_cache_miss() {
        let mut c: super::Xe66Cache<&str, i32> = super::Xe66Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_66_cache_ttl_expiry() {
        let mut c = super::Xe66Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_66_cache_evict() {
        let mut c = super::Xe66Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_66_cache_capacity() {
        let mut c = super::Xe66Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_66_cache_stats() {
        let mut c = super::Xe66Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_66_cache_clear() {
        let mut c = super::Xe66Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
