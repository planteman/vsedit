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
}
