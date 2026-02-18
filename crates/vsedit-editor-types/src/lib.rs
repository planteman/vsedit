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


// ---------------------------------------------------------------------------
// xg_64: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg64Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg64Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg64Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_64: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg64Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg64Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg64Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg64Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 38).
pub struct Xh38SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh38SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 80 as u64,
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

/// A compact bit set supporting boolean operations (variant 38).
pub struct Xh38BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh38BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 38).
pub struct Xi38Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi38Deque<T> {
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
pub struct Xi38Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi38Interval {
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

/// A simple interval tree (variant 38).
pub struct Xi38IntervalTree {
    xi_intervals: Vec<Xi38Interval>,
}

impl Xi38IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi38Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi38Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi38Interval) -> Vec<&Xi38Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi38Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi38Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi38Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi38Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi38Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi38Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 38) ---

/// Disjoint set / union-find for crate 38.
pub struct Xj38UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj38UnionFind {
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

const XJ38_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 38.
pub struct Xj38BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj38BTreeNode<K, V>>>,
    len: usize,
}

struct Xj38BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj38BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj38BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ38_BTREE_ORDER - 1
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
        let mid = XJ38_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj38BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj38BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj38BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj38BTreeNode::xj_new_leaf();
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


// --- xk_38 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk38SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk38SegmentTree {
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
pub struct Xk38DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk38DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_38).
#[derive(Debug, Clone)]
pub struct Xl38Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl38Rope {
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

/// Suffix array for efficient string searching (xl_38).
#[derive(Debug, Clone)]
pub struct Xl38SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl38SuffixArray {
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


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm38MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm38MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm38Tokenizer {
    text: String,
}

impl Xm38Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 38.
pub struct Xn38Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn38Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 38 -----

#[derive(Debug, Clone)]
struct Xn38AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn38AvlNode<K, V>>>,
    right: Option<Box<Xn38AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 38.
#[derive(Debug, Clone)]
pub struct Xn38AVL<K, V> {
    root: Option<Box<Xn38AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn38AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn38AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn38AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn38AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn38AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn38AvlNode<K, V>>) -> Box<Xn38AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn38AvlNode<K, V>>) -> Box<Xn38AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn38AvlNode<K, V>>) -> Box<Xn38AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn38AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn38AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn38AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn38AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn38AvlNode<K, V>>) -> &Xn38AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn38AvlNode<K, V>>) -> (Box<Xn38AvlNode<K, V>>, Option<Box<Xn38AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn38AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn38AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn38AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn38AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn38AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn38AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn38AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
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


    // -- xg_64 graph tests ------------------------------------------------

    #[test]
    fn xg_64_graph_empty() {
        let g = super::Xg64Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_64_graph_add_node() {
        let mut g = super::Xg64Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_64_graph_add_edge() {
        let mut g = super::Xg64Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_64_graph_neighbors() {
        let mut g = super::Xg64Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_64_graph_has_path() {
        let mut g = super::Xg64Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_64_graph_self_path() {
        let g = super::Xg64Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_64_graph_topo_sort() {
        let mut g = super::Xg64Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_64_graph_cycle_detect_false() {
        let mut g = super::Xg64Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_64_graph_cycle_detect_true() {
        let mut g = super::Xg64Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_64 heap tests -------------------------------------------------

    #[test]
    fn xg_64_heap_empty() {
        let h: super::Xg64Heap<i32> = super::Xg64Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_64_heap_push_pop() {
        let mut h = super::Xg64Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_64_heap_peek() {
        let mut h = super::Xg64Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_64_heap_drain_sorted() {
        let mut h = super::Xg64Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_64_heap_merge() {
        let mut a = super::Xg64Heap::new();
        let mut b = super::Xg64Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_64_heap_default() {
        let h: super::Xg64Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_64_graph_default() {
        let g: super::Xg64Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh38_skip_insert_contains() {
        let mut sl = super::Xh38SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh38_skip_remove() {
        let mut sl = super::Xh38SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh38_skip_len() {
        let mut sl = super::Xh38SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh38_skip_range_query() {
        let mut sl = super::Xh38SkipList::xh_new(4);
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
    fn xh38_skip_floor_ceiling() {
        let mut sl = super::Xh38SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh38_skip_rank() {
        let mut sl = super::Xh38SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh38_skip_empty() {
        let sl = super::Xh38SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh38_skip_duplicates() {
        let mut sl = super::Xh38SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh38_bitset_set_test() {
        let mut bs = super::Xh38BitSet::xh_new(256);
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
    fn xh38_bitset_clear_count() {
        let mut bs = super::Xh38BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh38_bitset_and_or_xor() {
        let mut a = super::Xh38BitSet::xh_new(128);
        let mut b = super::Xh38BitSet::xh_new(128);
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
    fn xh38_bitset_iter_ones() {
        let mut bs = super::Xh38BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh38_bitset_first_last() {
        let mut bs = super::Xh38BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh38_bitset_empty() {
        let bs = super::Xh38BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi38_deque_push_pop_back() {
        let mut dq = super::Xi38Deque::xi_new(4);
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
    fn xi38_deque_push_pop_front() {
        let mut dq = super::Xi38Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi38_deque_mixed_ops() {
        let mut dq = super::Xi38Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi38_deque_get_and_split() {
        let mut dq = super::Xi38Deque::xi_new(8);
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
    fn xi38_deque_rotate_left() {
        let mut dq = super::Xi38Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi38_deque_rotate_right() {
        let mut dq = super::Xi38Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi38_deque_grow() {
        let mut dq = super::Xi38Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi38_deque_empty() {
        let dq = super::Xi38Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi38_interval_tree_insert_query() {
        let mut tree = super::Xi38IntervalTree::xi_new();
        tree.xi_insert(super::Xi38Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi38Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi38Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi38_interval_tree_overlap() {
        let mut tree = super::Xi38IntervalTree::xi_new();
        tree.xi_insert(super::Xi38Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi38Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi38Interval::xi_new(12, 20));
        let q = super::Xi38Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi38_interval_tree_remove() {
        let mut tree = super::Xi38IntervalTree::xi_new();
        tree.xi_insert(super::Xi38Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi38Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi38_interval_tree_gaps() {
        let mut tree = super::Xi38IntervalTree::xi_new();
        tree.xi_insert(super::Xi38Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi38Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi38Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi38Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi38Interval::xi_new(8, 10));
    }

    #[test]
    fn xi38_interval_tree_merge() {
        let mut tree = super::Xi38IntervalTree::xi_new();
        tree.xi_insert(super::Xi38Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi38Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi38Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi38Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi38Interval::xi_new(10, 15));
    }

    #[test]
    fn xi38_interval_tree_all() {
        let mut tree = super::Xi38IntervalTree::xi_new();
        tree.xi_insert(super::Xi38Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi38Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi38_interval_tree_empty() {
        let tree = super::Xi38IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi38_interval_tree_contains_point() {
        let iv = super::Xi38Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 38) ---

    #[test]
    fn xj_38_uf_make_and_find() {
        let mut uf = super::Xj38UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_38_uf_union_connected() {
        let mut uf = super::Xj38UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_38_uf_component_count() {
        let mut uf = super::Xj38UnionFind::xj_new();
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
    fn xj_38_uf_component_size() {
        let mut uf = super::Xj38UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_38_uf_largest_component() {
        let mut uf = super::Xj38UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_38_uf_many_elements() {
        let mut uf = super::Xj38UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_38_uf_separate_components() {
        let mut uf = super::Xj38UnionFind::xj_new();
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
    fn xj_38_uf_path_compression() {
        let mut uf = super::Xj38UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_38_bt_insert_get() {
        let mut bt = super::Xj38BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_38_bt_contains_len() {
        let mut bt = super::Xj38BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_38_bt_replace() {
        let mut bt = super::Xj38BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_38_bt_remove() {
        let mut bt = super::Xj38BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_38_bt_keys_values() {
        let mut bt = super::Xj38BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_38_bt_range() {
        let mut bt = super::Xj38BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_38_bt_min_max() {
        let mut bt = super::Xj38BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_38_bt_many_inserts() {
        let mut bt = super::Xj38BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_38 segment tree tests ---

    #[test]
    fn xk_38_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk38SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_38_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk38SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_38_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk38SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_38_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk38SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_38_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk38SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_38_st_single_element() {
        let data = vec![42];
        let st = super::Xk38SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_38_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk38SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_38_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk38SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_38 disjoint intervals tests ---

    #[test]
    fn xk_38_di_add_and_count() {
        let mut di = super::Xk38DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_38_di_merge_overlap() {
        let mut di = super::Xk38DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_38_di_contains() {
        let mut di = super::Xk38DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_38_di_remove() {
        let mut di = super::Xk38DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_38_di_covered_length() {
        let mut di = super::Xk38DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_38_di_gaps() {
        let mut di = super::Xk38DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_38_di_merge_adjacent() {
        let mut di = super::Xk38DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_38_di_empty() {
        let di = super::Xk38DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_38_rope_new_empty() {
        let rope = super::Xl38Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_38_rope_from_str() {
        let rope = super::Xl38Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_38_rope_insert_at() {
        let mut rope = super::Xl38Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_38_rope_delete_range() {
        let mut rope = super::Xl38Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_38_rope_char_at() {
        let rope = super::Xl38Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_38_rope_split_concat() {
        let rope = super::Xl38Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_38_rope_line_count() {
        let rope = super::Xl38Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_38_rope_line_at() {
        let rope = super::Xl38Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_38_sa_build_and_search() {
        let sa = super::Xl38SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_38_sa_count() {
        let sa = super::Xl38SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_38_sa_longest_repeated() {
        let sa = super::Xl38SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_38_sa_all_positions() {
        let sa = super::Xl38SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_38_sa_len() {
        let sa = super::Xl38SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_38_sa_empty() {
        let sa = super::Xl38SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_38_rope_slice() {
        let rope = super::Xl38Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_38_sa_search_start() {
        let sa = super::Xl38SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_38_sparse_set_get() {
        let mut m = super::Xm38MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_38_sparse_row_col() {
        let mut m = super::Xm38MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_38_sparse_transpose() {
        let mut m = super::Xm38MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_38_sparse_multiply_vec() {
        let mut m = super::Xm38MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_38_sparse_nnz_density() {
        let mut m = super::Xm38MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_38_sparse_clear() {
        let mut m = super::Xm38MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_38_sparse_overwrite_zero() {
        let mut m = super::Xm38MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_38_tokenizer_basic() {
        let t = super::Xm38Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_38_tokenizer_count() {
        let t = super::Xm38Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_38_tokenizer_unique() {
        let t = super::Xm38Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_38_tokenizer_frequency() {
        let t = super::Xm38Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_38_tokenizer_delimiter() {
        let t = super::Xm38Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_38_tokenizer_whitespace() {
        let t = super::Xm38Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_38_tokenizer_empty() {
        let t = super::Xm38Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 38 ----

    #[test]
    fn xn_38_fenwick_prefix_sum() {
        let mut ft = super::Xn38Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_38_fenwick_range_sum() {
        let mut ft = super::Xn38Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_38_fenwick_point_query() {
        let mut ft = super::Xn38Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_38_fenwick_len() {
        let ft = super::Xn38Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_38_fenwick_multiple_updates() {
        let mut ft = super::Xn38Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_38_fenwick_single_element() {
        let mut ft = super::Xn38Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_38_fenwick_find_kth() {
        let mut ft = super::Xn38Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_38_fenwick_negative_delta() {
        let mut ft = super::Xn38Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 38 ----

    #[test]
    fn xn_38_avl_insert_get() {
        let mut m = super::Xn38AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_38_avl_remove() {
        let mut m = super::Xn38AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_38_avl_in_order() {
        let mut m = super::Xn38AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_38_avl_min_max() {
        let mut m = super::Xn38AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_38_avl_floor_ceiling() {
        let mut m = super::Xn38AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_38_avl_height_balanced() {
        let mut m = super::Xn38AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_38_avl_overwrite() {
        let mut m = super::Xn38AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_38_avl_empty() {
        let m: super::Xn38AVL<i32, i32> = super::Xn38AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
