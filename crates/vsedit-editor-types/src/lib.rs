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
        write!(f, "({}, {})", self.line, self.column)
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
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} -> {}]", self.start, self.end)
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
        assert_eq!(Position::new(1, 5).to_string(), "(1, 5)");
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
        assert_eq!(Range::new(1, 1, 2, 5).to_string(), "[(1, 1) -> (2, 5)]");
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
}
