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
}
