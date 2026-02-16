//! Multi-cursor operations.
//!
//! Provides lightweight cursor position tracking, selection ranges,
//! and column-selection mode utilities that complement the lower-level
//! [`vsedit_cursor::CursorController`].

/// A position in a text document (1-based line and column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CursorPosition {
    pub line: u32,
    pub column: u32,
}

impl CursorPosition {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// A contiguous selection between two positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: CursorPosition,
    pub end: CursorPosition,
}

impl Selection {
    pub fn new(start: CursorPosition, end: CursorPosition) -> Self {
        Self { start, end }
    }

    /// Returns `true` when start equals end (no text is selected).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Manages a set of cursors and their associated selections.
#[derive(Debug, Clone)]
pub struct MultiCursorSession {
    pub cursors: Vec<CursorPosition>,
    pub selections: Vec<Selection>,
}

impl MultiCursorSession {
    /// Create a session with no cursors.
    pub fn new() -> Self {
        Self {
            cursors: Vec::new(),
            selections: Vec::new(),
        }
    }

    /// Add a cursor at the given position.
    pub fn add_cursor(&mut self, pos: CursorPosition) {
        self.cursors.push(pos);
    }

    /// Remove the cursor at `index`. Returns `None` if out of bounds.
    pub fn remove_cursor(&mut self, index: usize) -> Option<CursorPosition> {
        if index < self.cursors.len() {
            Some(self.cursors.remove(index))
        } else {
            None
        }
    }

    /// Add a cursor one line above the first cursor, keeping the same column.
    /// `max_column_fn` clamps the column to the target line's width.
    pub fn add_cursor_above(&mut self, max_column_fn: impl Fn(u32) -> u32) {
        if let Some(first) = self.cursors.first().copied() {
            if first.line > 1 {
                let new_line = first.line - 1;
                let col = first.column.min(max_column_fn(new_line));
                self.cursors.push(CursorPosition::new(new_line, col));
            }
        }
    }

    /// Add a cursor one line below the last cursor, keeping the same column.
    /// `max_column_fn` clamps the column; `line_count` is the total number of lines.
    pub fn add_cursor_below(
        &mut self,
        line_count: u32,
        max_column_fn: impl Fn(u32) -> u32,
    ) {
        if let Some(last) = self.cursors.last().copied() {
            if last.line < line_count {
                let new_line = last.line + 1;
                let col = last.column.min(max_column_fn(new_line));
                self.cursors.push(CursorPosition::new(new_line, col));
            }
        }
    }

    /// Sort cursors by position and remove duplicates.
    pub fn sort_and_deduplicate(&mut self) {
        self.cursors.sort();
        self.cursors.dedup();
    }

    /// Number of active cursors.
    pub fn cursor_count(&self) -> usize {
        self.cursors.len()
    }

    /// Remove all cursors and selections.
    pub fn clear(&mut self) {
        self.cursors.clear();
        self.selections.clear();
    }

    /// Returns `true` when more than one cursor is active.
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }
}

impl Default for MultiCursorSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility for computing column-aligned selections across a range of lines.
#[derive(Debug, Clone)]
pub struct ColumnSelectionMode {
    pub anchor_column: u32,
}

impl ColumnSelectionMode {
    pub fn new(anchor_column: u32) -> Self {
        Self { anchor_column }
    }

    /// Compute one [`Selection`] per line in `start_line..=end_line`.
    ///
    /// `max_column_fn` returns the maximum valid column for a given line.
    /// The selection on each line spans from `anchor_column` to `target_column`,
    /// both clamped to the line width.
    pub fn compute_selections(
        &self,
        start_line: u32,
        end_line: u32,
        target_column: u32,
        max_column_fn: impl Fn(u32) -> u32,
    ) -> Vec<Selection> {
        let (lo, hi) = if start_line <= end_line {
            (start_line, end_line)
        } else {
            (end_line, start_line)
        };

        (lo..=hi)
            .map(|line| {
                let max_col = max_column_fn(line);
                let a = self.anchor_column.min(max_col);
                let b = target_column.min(max_col);
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                Selection::new(
                    CursorPosition::new(line, start),
                    CursorPosition::new(line, end),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_cursors() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        session.add_cursor(CursorPosition::new(2, 5));
        assert_eq!(session.cursor_count(), 2);
        assert!(session.has_multiple_cursors());

        let removed = session.remove_cursor(0);
        assert_eq!(removed, Some(CursorPosition::new(1, 1)));
        assert_eq!(session.cursor_count(), 1);
        assert!(!session.has_multiple_cursors());
    }

    #[test]
    fn sort_and_deduplicate_removes_dups() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(3, 1));
        session.add_cursor(CursorPosition::new(1, 1));
        session.add_cursor(CursorPosition::new(3, 1)); // duplicate
        session.sort_and_deduplicate();
        assert_eq!(session.cursor_count(), 2);
        assert_eq!(session.cursors[0], CursorPosition::new(1, 1));
        assert_eq!(session.cursors[1], CursorPosition::new(3, 1));
    }

    #[test]
    fn add_cursor_above_and_below() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(3, 10));

        // Line widths: line 2 has max col 5, line 4 has max col 15
        session.add_cursor_above(|_| 5);
        assert_eq!(session.cursor_count(), 2);
        assert_eq!(session.cursors[1], CursorPosition::new(2, 5));

        // add_cursor_below uses the last cursor (line 2, col 5)
        session.add_cursor_below(10, |_| 15);
        assert_eq!(session.cursor_count(), 3);
        assert_eq!(session.cursors[2], CursorPosition::new(3, 5));
    }

    #[test]
    fn column_selection_mode() {
        let csm = ColumnSelectionMode::new(3);
        // 3 lines, target column 8, all lines have max col 10
        let sels = csm.compute_selections(1, 3, 8, |_| 10);
        assert_eq!(sels.len(), 3);
        for sel in &sels {
            assert_eq!(sel.start.column, 3);
            assert_eq!(sel.end.column, 8);
        }

        // Short line clamps both anchor and target
        let sels = csm.compute_selections(1, 1, 8, |_| 4);
        assert_eq!(sels[0].start.column, 3);
        assert_eq!(sels[0].end.column, 4);
    }

    #[test]
    fn clear_removes_everything() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        session.selections.push(Selection::new(
            CursorPosition::new(1, 1),
            CursorPosition::new(1, 5),
        ));
        session.clear();
        assert_eq!(session.cursor_count(), 0);
        assert!(session.selections.is_empty());
    }
}
