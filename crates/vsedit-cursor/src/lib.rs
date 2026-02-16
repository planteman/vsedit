//! Multi-cursor controller for vsedit.
//!
//! Equivalent to VS Code's `vs/editor/common/cursorCommon.ts` and related files.
//! Provides [`CursorState`], [`CursorController`], and standalone cursor movement
//! functions that operate on a single cursor against an [`ITextModel`].

use vsedit_editor_types::{ITextModel, Position, Selection};

// ---------------------------------------------------------------------------
// CursorState
// ---------------------------------------------------------------------------

/// A single cursor with its selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorState {
    pub selection: Selection,
}

impl CursorState {
    /// Create a collapsed cursor at the given position.
    pub fn from_position(pos: Position) -> Self {
        Self {
            selection: Selection::from_positions(pos, pos),
        }
    }

    /// The position where the cursor is rendered (the active end of the selection).
    pub fn position(&self) -> Position {
        self.selection.active
    }
}

// ---------------------------------------------------------------------------
// CursorController
// ---------------------------------------------------------------------------

/// Manages multiple cursors within an editor.
pub struct CursorController {
    cursors: Vec<CursorState>,
    /// Column memory for vertical movement (one per cursor).
    column_select_data: Vec<Option<u32>>,
}

impl CursorController {
    /// Create a controller with a single cursor at line 1, column 1.
    pub fn new() -> Self {
        let state = CursorState::from_position(Position::new(1, 1));
        Self {
            cursors: vec![state],
            column_select_data: vec![None],
        }
    }

    /// Create a controller with the primary cursor at the given position.
    pub fn from_position(pos: Position) -> Self {
        let state = CursorState::from_position(pos);
        Self {
            cursors: vec![state],
            column_select_data: vec![None],
        }
    }

    /// The primary (first) cursor.
    pub fn get_primary(&self) -> &CursorState {
        &self.cursors[0]
    }

    /// All cursors.
    pub fn get_all(&self) -> &[CursorState] {
        &self.cursors
    }

    /// Add a secondary cursor at the given position.
    pub fn add_cursor(&mut self, position: Position) {
        self.cursors
            .push(CursorState::from_position(position));
        self.column_select_data.push(None);
    }

    /// Add a cursor one line above the primary cursor (Ctrl+Alt+Up).
    pub fn add_cursor_above(&mut self, model: &dyn ITextModel) {
        let primary = &self.cursors[0];
        let line = primary.position().line;
        if line <= 1 {
            return;
        }
        let new_line = line - 1;
        let col = primary
            .position()
            .column
            .min(model.get_line_max_column(new_line));
        self.cursors
            .push(CursorState::from_position(Position::new(new_line, col)));
        self.column_select_data.push(None);
    }

    /// Add a cursor one line below the primary cursor (Ctrl+Alt+Down).
    pub fn add_cursor_below(&mut self, model: &dyn ITextModel) {
        let primary = &self.cursors[0];
        let line = primary.position().line;
        if line >= model.get_line_count() {
            return;
        }
        let new_line = line + 1;
        let col = primary
            .position()
            .column
            .min(model.get_line_max_column(new_line));
        self.cursors
            .push(CursorState::from_position(Position::new(new_line, col)));
        self.column_select_data.push(None);
    }

    /// Merge cursors whose selections overlap or touch, keeping the earlier one.
    pub fn merge_overlapping(&mut self) {
        if self.cursors.len() <= 1 {
            return;
        }

        // Build (index, range) pairs sorted by range start.
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by(|&a, &b| {
            let ra = self.cursors[a].selection.as_range();
            let rb = self.cursors[b].selection.as_range();
            ra.start.cmp(&rb.start).then(ra.end.cmp(&rb.end))
        });

        let mut keep = vec![true; self.cursors.len()];
        let mut prev = indices[0];
        for &idx in &indices[1..] {
            let prev_range = self.cursors[prev].selection.as_range();
            let cur_range = self.cursors[idx].selection.as_range();
            // Overlapping or touching (position equality counts as touching).
            if cur_range.start <= prev_range.end {
                // Merge into prev: expand prev range, drop current.
                let merged_anchor =
                    Position::min(prev_range.start, cur_range.start);
                let merged_active =
                    Position::max(prev_range.end, cur_range.end);
                self.cursors[prev].selection =
                    Selection::from_positions(merged_anchor, merged_active);
                keep[idx] = false;
            } else {
                prev = idx;
            }
        }

        let mut new_cursors = Vec::new();
        let mut new_col = Vec::new();
        for (i, cursor) in self.cursors.iter().enumerate() {
            if keep[i] {
                new_cursors.push(cursor.clone());
                new_col.push(self.column_select_data[i]);
            }
        }
        self.cursors = new_cursors;
        self.column_select_data = new_col;
    }

    /// Set the column memory for cursor at `index`.
    pub fn set_column_memory(&mut self, index: usize, col: Option<u32>) {
        if index < self.column_select_data.len() {
            self.column_select_data[index] = col;
        }
    }

    /// Get the column memory for cursor at `index`.
    pub fn get_column_memory(&self, index: usize) -> Option<u32> {
        self.column_select_data.get(index).copied().flatten()
    }

    /// Set the state for cursor at `index`.
    pub fn set_state(&mut self, index: usize, state: CursorState) {
        if index < self.cursors.len() {
            self.cursors[index] = state;
        }
    }

    /// Remove all secondary cursors, keeping only the primary cursor.
    pub fn remove_secondary_cursors(&mut self) {
        self.cursors.truncate(1);
        self.column_select_data.truncate(1);
    }

    /// Return `true` when more than one cursor exists.
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }

    /// Undo the last cursor addition (soft undo).
    pub fn cursor_undo(&mut self) {
        if self.cursors.len() > 1 {
            self.cursors.pop();
            self.column_select_data.pop();
        }
    }
}

impl Default for CursorController {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cursor movement (single-cursor, pure functions)
// ---------------------------------------------------------------------------

/// Build a new `CursorState` by moving the active position.
/// If `select` is true the anchor stays (extending the selection); otherwise
/// the selection collapses to the new active position.
fn make_state(cursor: &CursorState, select: bool, new_active: Position) -> CursorState {
    let anchor = if select {
        cursor.selection.anchor
    } else {
        new_active
    };
    CursorState {
        selection: Selection::from_positions(anchor, new_active),
    }
}

/// Move the cursor left by `columns` columns, wrapping to the previous line.
pub fn move_left(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
    columns: u32,
) -> CursorState {
    let mut line = cursor.position().line;
    let mut col = cursor.position().column;
    let mut remaining = columns;

    while remaining > 0 {
        if col > 1 {
            let step = remaining.min(col - 1);
            col -= step;
            remaining -= step;
        } else if line > 1 {
            line -= 1;
            col = model.get_line_max_column(line);
            remaining -= 1;
        } else {
            break;
        }
    }

    make_state(cursor, select, Position::new(line, col))
}

/// Move the cursor right by `columns` columns, wrapping to the next line.
pub fn move_right(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
    columns: u32,
) -> CursorState {
    let line_count = model.get_line_count();
    let mut line = cursor.position().line;
    let mut col = cursor.position().column;
    let mut remaining = columns;

    while remaining > 0 {
        let max_col = model.get_line_max_column(line);
        if col < max_col {
            let step = remaining.min(max_col - col);
            col += step;
            remaining -= step;
        } else if line < line_count {
            line += 1;
            col = 1;
            remaining -= 1;
        } else {
            break;
        }
    }

    make_state(cursor, select, Position::new(line, col))
}

/// Move the cursor up by `lines` lines, keeping column memory.
///
/// `column_memory` should be the remembered desired column (or `None` to use
/// the current column). Returns `(new_state, new_column_memory)`.
pub fn move_up(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
    lines: u32,
    column_memory: Option<u32>,
) -> (CursorState, u32) {
    let desired_col = column_memory.unwrap_or(cursor.position().column);
    let cur_line = cursor.position().line;
    let new_line = if cur_line > lines {
        cur_line - lines
    } else {
        1
    };
    let max_col = model.get_line_max_column(new_line);
    let col = desired_col.min(max_col);
    (
        make_state(cursor, select, Position::new(new_line, col)),
        desired_col,
    )
}

/// Move the cursor down by `lines` lines, keeping column memory.
///
/// Returns `(new_state, new_column_memory)`.
pub fn move_down(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
    lines: u32,
    column_memory: Option<u32>,
) -> (CursorState, u32) {
    let desired_col = column_memory.unwrap_or(cursor.position().column);
    let cur_line = cursor.position().line;
    let line_count = model.get_line_count();
    let new_line = (cur_line + lines).min(line_count);
    let max_col = model.get_line_max_column(new_line);
    let col = desired_col.min(max_col);
    (
        make_state(cursor, select, Position::new(new_line, col)),
        desired_col,
    )
}

/// Move to the beginning of the line. Implements VS Code "Home" toggle:
/// first press goes to the first non-whitespace character; if already there,
/// goes to column 1.
pub fn move_to_line_start(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
) -> CursorState {
    let line = cursor.position().line;
    let content = model.get_line_content(line);

    let first_non_ws = content
        .bytes()
        .position(|b| !b.is_ascii_whitespace())
        .map(|i| (i as u32) + 1)
        .unwrap_or(1);

    let target_col = if cursor.position().column == first_non_ws {
        1
    } else {
        first_non_ws
    };

    make_state(cursor, select, Position::new(line, target_col))
}

/// Move to the end of the current line.
pub fn move_to_line_end(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
) -> CursorState {
    let line = cursor.position().line;
    let max_col = model.get_line_max_column(line);
    make_state(cursor, select, Position::new(line, max_col))
}

/// Move to the start of the document (line 1, column 1).
pub fn move_to_document_start(
    _model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
) -> CursorState {
    make_state(cursor, select, Position::new(1, 1))
}

/// Move to the end of the document.
pub fn move_to_document_end(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
) -> CursorState {
    let last_line = model.get_line_count();
    let max_col = model.get_line_max_column(last_line);
    make_state(cursor, select, Position::new(last_line, max_col))
}

// ---------------------------------------------------------------------------
// Word classification (VS Code rules with camelCase support)
// ---------------------------------------------------------------------------

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum CharClass {
    Uppercase,
    Lowercase,
    Digit,
    Underscore,
    Whitespace,
    Separator,
}

fn classify_char(ch: u8) -> CharClass {
    match ch {
        b'A'..=b'Z' => CharClass::Uppercase,
        b'a'..=b'z' => CharClass::Lowercase,
        b'0'..=b'9' => CharClass::Digit,
        b'_' => CharClass::Underscore,
        b' ' | b'\t' | b'\r' | b'\n' => CharClass::Whitespace,
        _ => CharClass::Separator,
    }
}

/// Returns true if there is a word boundary between `left` and `right` chars
/// (VS Code algorithm with camelCase support).
#[allow(dead_code)]
fn is_word_boundary(left: u8, right: u8) -> bool {
    let lc = classify_char(left);
    let rc = classify_char(right);
    if lc == rc {
        return false;
    }
    // camelCase boundary: lowercase→uppercase
    if lc == CharClass::Lowercase && rc == CharClass::Uppercase {
        return true;
    }
    // Underscore groups with nothing (always a boundary with non-underscore)
    // but sequences of underscores are one group
    if lc == CharClass::Underscore && rc == CharClass::Underscore {
        return false;
    }
    // Same "word-like" group: uppercase+lowercase (e.g. mid-word in PascalCase
    // like "HTMLParser" — "HTMLP" then "arser": boundary before 'a' because
    // uppercase→lowercase when preceded by multiple uppercase)
    // This is handled by the movement functions with lookahead.
    lc != rc
}

/// Move the cursor to the start of the previous word (VS Code Ctrl+Left).
pub fn move_word_left(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
) -> CursorState {
    let mut line = cursor.position().line;
    let mut col = cursor.position().column;

    loop {
        let content = model.get_line_content(line);
        let bytes = content.as_bytes();
        let mut idx = (col as usize).saturating_sub(1);

        if idx == 0 {
            if line > 1 {
                line -= 1;
                col = model.get_line_max_column(line);
                continue;
            }
            break;
        }

        // Skip whitespace to the left.
        while idx > 0 && classify_char(bytes[idx - 1]) == CharClass::Whitespace {
            idx -= 1;
        }

        if idx == 0 {
            col = 1;
            break;
        }

        let cls = classify_char(bytes[idx - 1]);
        if cls == CharClass::Separator || cls == CharClass::Underscore {
            // Consume contiguous separators/underscores
            while idx > 0 && classify_char(bytes[idx - 1]) == cls {
                idx -= 1;
            }
        } else if cls == CharClass::Lowercase || cls == CharClass::Digit {
            while idx > 0 && classify_char(bytes[idx - 1]) == cls {
                idx -= 1;
            }
            // If preceded by uppercase (camelCase prefix), consume one uppercase
            if idx > 0 && classify_char(bytes[idx - 1]) == CharClass::Uppercase {
                idx -= 1;
            }
        } else if cls == CharClass::Uppercase {
            // Consume uppercase run
            while idx > 0 && classify_char(bytes[idx - 1]) == CharClass::Uppercase {
                idx -= 1;
            }
        }

        col = (idx as u32) + 1;
        break;
    }

    make_state(cursor, select, Position::new(line, col))
}

/// Move the cursor to the end of the next word (VS Code Ctrl+Right).
pub fn move_word_right(
    model: &dyn ITextModel,
    cursor: &CursorState,
    select: bool,
) -> CursorState {
    let line_count = model.get_line_count();
    let mut line = cursor.position().line;
    let mut col = cursor.position().column;

    loop {
        let content = model.get_line_content(line);
        let bytes = content.as_bytes();
        let len = bytes.len();
        let mut idx = (col as usize).saturating_sub(1);

        if idx >= len {
            if line < line_count {
                line += 1;
                col = 1;
                continue;
            }
            break;
        }

        // Skip whitespace to the right.
        while idx < len && classify_char(bytes[idx]) == CharClass::Whitespace {
            idx += 1;
        }

        if idx >= len {
            col = model.get_line_max_column(line);
            break;
        }

        let cls = classify_char(bytes[idx]);
        if cls == CharClass::Separator || cls == CharClass::Underscore {
            while idx < len && classify_char(bytes[idx]) == cls {
                idx += 1;
            }
        } else if cls == CharClass::Uppercase {
            // Consume uppercase run
            let start = idx;
            while idx < len && classify_char(bytes[idx]) == CharClass::Uppercase {
                idx += 1;
            }
            // If followed by lowercase (e.g. "HTMLParser" → stop before last upper "P")
            if idx < len && classify_char(bytes[idx]) == CharClass::Lowercase && idx - start > 1 {
                idx -= 1;
            }
            // Then consume lowercase/digits
            if idx < len && (classify_char(bytes[idx]) == CharClass::Lowercase
                || classify_char(bytes[idx]) == CharClass::Digit)
            {
                while idx < len && (classify_char(bytes[idx]) == CharClass::Lowercase
                    || classify_char(bytes[idx]) == CharClass::Digit)
                {
                    idx += 1;
                }
            }
        } else {
            // Lowercase or digit run
            while idx < len && classify_char(bytes[idx]) == cls {
                idx += 1;
            }
        }

        col = (idx as u32) + 1;
        break;
    }

    make_state(cursor, select, Position::new(line, col))
}

/// Delete the text from cursor to the word boundary to the left.
/// Returns `(new_text, new_position)` for the line.
pub fn delete_word_left(
    model: &dyn ITextModel,
    cursor: &CursorState,
) -> (Position, Position) {
    let word_start = move_word_left(model, cursor, false);
    (word_start.position(), cursor.position())
}

/// Delete the text from cursor to the word boundary to the right.
/// Returns `(start, end)` positions for the range to delete.
pub fn delete_word_right(
    model: &dyn ITextModel,
    cursor: &CursorState,
) -> (Position, Position) {
    let word_end = move_word_right(model, cursor, false);
    (cursor.position(), word_end.position())
}

/// Select the word at the given cursor position (double-click behavior).
/// Returns a CursorState with the word selected, or the original if no word.
pub fn select_word_at(
    model: &dyn ITextModel,
    cursor: &CursorState,
) -> CursorState {
    let pos = cursor.position();
    let content = model.get_line_content(pos.line);
    let bytes = content.as_bytes();
    let col_idx = (pos.column as usize).saturating_sub(1);

    if bytes.is_empty() || col_idx >= bytes.len() {
        return cursor.clone();
    }

    let ch_class = classify_char(bytes[col_idx]);
    if ch_class == CharClass::Whitespace {
        // Select whitespace run
        let mut start = col_idx;
        let mut end = col_idx;
        while start > 0 && classify_char(bytes[start - 1]) == CharClass::Whitespace {
            start -= 1;
        }
        while end < bytes.len() && classify_char(bytes[end]) == CharClass::Whitespace {
            end += 1;
        }
        return CursorState {
            selection: Selection::from_positions(
                Position::new(pos.line, (start as u32) + 1),
                Position::new(pos.line, (end as u32) + 1),
            ),
        };
    }

    if ch_class == CharClass::Separator {
        // Select separator run
        let mut start = col_idx;
        let mut end = col_idx;
        while start > 0 && classify_char(bytes[start - 1]) == CharClass::Separator {
            start -= 1;
        }
        while end < bytes.len() && classify_char(bytes[end]) == CharClass::Separator {
            end += 1;
        }
        return CursorState {
            selection: Selection::from_positions(
                Position::new(pos.line, (start as u32) + 1),
                Position::new(pos.line, (end as u32) + 1),
            ),
        };
    }

    // Word character (upper, lower, digit, underscore) — select word
    let is_word = |b: u8| {
        let c = classify_char(b);
        c == CharClass::Uppercase || c == CharClass::Lowercase
            || c == CharClass::Digit || c == CharClass::Underscore
    };

    let mut start = col_idx;
    let mut end = col_idx;
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    while end < bytes.len() && is_word(bytes[end]) {
        end += 1;
    }

    CursorState {
        selection: Selection::from_positions(
            Position::new(pos.line, (start as u32) + 1),
            Position::new(pos.line, (end as u32) + 1),
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial `ITextModel` for testing.
    struct SimpleModel {
        lines: Vec<String>,
    }

    impl SimpleModel {
        fn new(text: &str) -> Self {
            Self {
                lines: text.split('\n').map(String::from).collect(),
            }
        }
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

    fn cursor_at(line: u32, col: u32) -> CursorState {
        CursorState::from_position(Position::new(line, col))
    }

    // -- Left wrapping ------------------------------------------------------

    #[test]
    fn left_wraps_to_previous_line() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(2, 1);
        let result = move_left(&model, &c, false, 1);
        assert_eq!(result.position(), Position::new(1, 6));
    }

    #[test]
    fn left_stops_at_document_start() {
        let model = SimpleModel::new("hello");
        let c = cursor_at(1, 1);
        let result = move_left(&model, &c, false, 5);
        assert_eq!(result.position(), Position::new(1, 1));
    }

    #[test]
    fn left_multiple_columns() {
        let model = SimpleModel::new("hello");
        let c = cursor_at(1, 4);
        let result = move_left(&model, &c, false, 2);
        assert_eq!(result.position(), Position::new(1, 2));
    }

    // -- Right wrapping -----------------------------------------------------

    #[test]
    fn right_wraps_to_next_line() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 6);
        let result = move_right(&model, &c, false, 1);
        assert_eq!(result.position(), Position::new(2, 1));
    }

    #[test]
    fn right_stops_at_document_end() {
        let model = SimpleModel::new("hello");
        let c = cursor_at(1, 6);
        let result = move_right(&model, &c, false, 10);
        assert_eq!(result.position(), Position::new(1, 6));
    }

    #[test]
    fn right_multiple_columns() {
        let model = SimpleModel::new("hello");
        let c = cursor_at(1, 1);
        let result = move_right(&model, &c, false, 3);
        assert_eq!(result.position(), Position::new(1, 4));
    }

    // -- Up / Down with column memory ---------------------------------------

    #[test]
    fn up_basic() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(2, 3);
        let (result, _) = move_up(&model, &c, false, 1, None);
        assert_eq!(result.position(), Position::new(1, 3));
    }

    #[test]
    fn down_basic() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 3);
        let (result, _) = move_down(&model, &c, false, 1, None);
        assert_eq!(result.position(), Position::new(2, 3));
    }

    #[test]
    fn up_down_column_memory_through_short_line() {
        // Lines: "long line" (10 chars), "hi" (2 chars), "long line" (10 chars)
        let model = SimpleModel::new("long line!\nhi\nlong line!");
        let c = cursor_at(1, 10); // column 10 in first line

        // Move down: line 2 is short, so column clamps to 3 (max_col of "hi")
        let (c2, mem) = move_down(&model, &c, false, 1, None);
        assert_eq!(c2.position(), Position::new(2, 3));
        assert_eq!(mem, 10); // remember column 10

        // Move down again with memory: line 3 is long enough
        let (c3, mem2) = move_down(&model, &c2, false, 1, Some(mem));
        assert_eq!(c3.position(), Position::new(3, 10));
        assert_eq!(mem2, 10);
    }

    #[test]
    fn up_stops_at_line_1() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 3);
        let (result, _) = move_up(&model, &c, false, 5, None);
        assert_eq!(result.position(), Position::new(1, 3));
    }

    #[test]
    fn down_stops_at_last_line() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(2, 3);
        let (result, _) = move_down(&model, &c, false, 5, None);
        assert_eq!(result.position(), Position::new(2, 3));
    }

    // -- Word movement ------------------------------------------------------

    #[test]
    fn word_right_basic() {
        let model = SimpleModel::new("hello world");
        let c = cursor_at(1, 1);
        let result = move_word_right(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 6)); // end of "hello"
    }

    #[test]
    fn word_left_basic() {
        let model = SimpleModel::new("hello world");
        let c = cursor_at(1, 12);
        let result = move_word_left(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 7)); // start of "world"
    }

    #[test]
    fn word_right_skips_whitespace() {
        let model = SimpleModel::new("hello   world");
        let c = cursor_at(1, 6); // after "hello"
        let result = move_word_right(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 14)); // end of "world"
    }

    #[test]
    fn word_left_skips_whitespace() {
        let model = SimpleModel::new("hello   world");
        let c = cursor_at(1, 9); // in whitespace before "world"
        let result = move_word_left(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 1)); // start of "hello"
    }

    #[test]
    fn word_right_separators() {
        let model = SimpleModel::new("foo.bar");
        let c = cursor_at(1, 1);
        let r = move_word_right(&model, &c, false);
        assert_eq!(r.position(), Position::new(1, 4)); // end of "foo"
        let r2 = move_word_right(&model, &r, false);
        assert_eq!(r2.position(), Position::new(1, 5)); // past "."
        let r3 = move_word_right(&model, &r2, false);
        assert_eq!(r3.position(), Position::new(1, 8)); // end of "bar"
    }

    #[test]
    fn word_left_wraps_line() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(2, 1);
        let result = move_word_left(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 1));
    }

    #[test]
    fn word_right_wraps_line() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 6); // past end of line 1
        let result = move_word_right(&model, &c, false);
        assert_eq!(result.position(), Position::new(2, 6)); // end of "world"
    }

    // -- Home toggle --------------------------------------------------------

    #[test]
    fn home_goes_to_first_non_whitespace() {
        let model = SimpleModel::new("    hello");
        let c = cursor_at(1, 10); // end of line
        let result = move_to_line_start(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 5)); // first non-ws
    }

    #[test]
    fn home_toggle_to_column_1() {
        let model = SimpleModel::new("    hello");
        let c = cursor_at(1, 5); // already at first non-ws
        let result = move_to_line_start(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 1)); // column 1
    }

    #[test]
    fn home_toggle_back_to_non_whitespace() {
        let model = SimpleModel::new("    hello");
        let c = cursor_at(1, 1); // at column 1
        let result = move_to_line_start(&model, &c, false);
        // Column 1 != first_non_ws (5), so goes to 5
        assert_eq!(result.position(), Position::new(1, 5));
    }

    // -- Line end / Document start/end --------------------------------------

    #[test]
    fn line_end() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 1);
        let result = move_to_line_end(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 6));
    }

    #[test]
    fn document_start() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(2, 3);
        let result = move_to_document_start(&model, &c, false);
        assert_eq!(result.position(), Position::new(1, 1));
    }

    #[test]
    fn document_end() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 1);
        let result = move_to_document_end(&model, &c, false);
        assert_eq!(result.position(), Position::new(2, 6));
    }

    // -- Select mode --------------------------------------------------------

    #[test]
    fn select_right_extends_selection() {
        let model = SimpleModel::new("hello");
        let c = cursor_at(1, 1);
        let result = move_right(&model, &c, true, 3);
        assert_eq!(result.selection.anchor, Position::new(1, 1));
        assert_eq!(result.selection.active, Position::new(1, 4));
    }

    #[test]
    fn select_left_extends_selection() {
        let model = SimpleModel::new("hello");
        let c = cursor_at(1, 4);
        let result = move_left(&model, &c, true, 2);
        assert_eq!(result.selection.anchor, Position::new(1, 4));
        assert_eq!(result.selection.active, Position::new(1, 2));
    }

    #[test]
    fn select_down_extends_selection() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 3);
        let (result, _) = move_down(&model, &c, true, 1, None);
        assert_eq!(result.selection.anchor, Position::new(1, 3));
        assert_eq!(result.selection.active, Position::new(2, 3));
    }

    #[test]
    fn select_word_right_extends() {
        let model = SimpleModel::new("hello world");
        let c = cursor_at(1, 1);
        let result = move_word_right(&model, &c, true);
        assert_eq!(result.selection.anchor, Position::new(1, 1));
        assert_eq!(result.selection.active, Position::new(1, 6));
    }

    #[test]
    fn select_home_extends() {
        let model = SimpleModel::new("    hello");
        let c = cursor_at(1, 10);
        let result = move_to_line_start(&model, &c, true);
        assert_eq!(result.selection.anchor, Position::new(1, 10));
        assert_eq!(result.selection.active, Position::new(1, 5));
    }

    #[test]
    fn select_document_end_extends() {
        let model = SimpleModel::new("hello\nworld");
        let c = cursor_at(1, 1);
        let result = move_to_document_end(&model, &c, true);
        assert_eq!(result.selection.anchor, Position::new(1, 1));
        assert_eq!(result.selection.active, Position::new(2, 6));
    }

    // -- Multi-cursor merge -------------------------------------------------

    #[test]
    fn merge_overlapping_cursors() {
        let mut ctrl = CursorController::new();
        ctrl.cursors[0] = CursorState {
            selection: Selection::new(1, 1, 1, 5),
        };
        ctrl.column_select_data = vec![None];
        ctrl.add_cursor(Position::new(1, 3)); // overlaps with [1,1)–[1,5)
        ctrl.add_cursor(Position::new(2, 1)); // separate

        ctrl.merge_overlapping();
        assert_eq!(ctrl.get_all().len(), 2);
        // First merged cursor should span 1,1 -> 1,5
        assert_eq!(ctrl.get_primary().selection.anchor, Position::new(1, 1));
        assert_eq!(ctrl.get_primary().selection.active, Position::new(1, 5));
    }

    #[test]
    fn merge_no_overlap_keeps_all() {
        let mut ctrl = CursorController::new();
        ctrl.cursors[0] = cursor_at(1, 1);
        ctrl.add_cursor(Position::new(2, 1));
        ctrl.add_cursor(Position::new(3, 1));
        ctrl.merge_overlapping();
        assert_eq!(ctrl.get_all().len(), 3);
    }

    // -- CursorController add_cursor_above / below --------------------------

    #[test]
    fn add_cursor_above() {
        let model = SimpleModel::new("hello\nworld\nfoo");
        let mut ctrl = CursorController::from_position(Position::new(2, 3));
        ctrl.add_cursor_above(&model);
        assert_eq!(ctrl.get_all().len(), 2);
        assert_eq!(ctrl.get_all()[1].position(), Position::new(1, 3));
    }

    #[test]
    fn add_cursor_below() {
        let model = SimpleModel::new("hello\nworld\nfoo");
        let mut ctrl = CursorController::from_position(Position::new(2, 3));
        ctrl.add_cursor_below(&model);
        assert_eq!(ctrl.get_all().len(), 2);
        assert_eq!(ctrl.get_all()[1].position(), Position::new(3, 3));
    }

    #[test]
    fn add_cursor_above_clamps_column() {
        let model = SimpleModel::new("hi\nlong line");
        let mut ctrl = CursorController::from_position(Position::new(2, 10));
        ctrl.add_cursor_above(&model);
        // Line 1 "hi" max_col = 3
        assert_eq!(ctrl.get_all()[1].position(), Position::new(1, 3));
    }

    #[test]
    fn add_cursor_above_at_line_1_noop() {
        let model = SimpleModel::new("hello");
        let mut ctrl = CursorController::from_position(Position::new(1, 1));
        ctrl.add_cursor_above(&model);
        assert_eq!(ctrl.get_all().len(), 1);
    }

    #[test]
    fn add_cursor_below_at_last_line_noop() {
        let model = SimpleModel::new("hello");
        let mut ctrl = CursorController::from_position(Position::new(1, 1));
        ctrl.add_cursor_below(&model);
        assert_eq!(ctrl.get_all().len(), 1);
    }

    // -- Column memory via controller ---------------------------------------

    #[test]
    fn controller_column_memory() {
        let mut ctrl = CursorController::new();
        assert_eq!(ctrl.get_column_memory(0), None);
        ctrl.set_column_memory(0, Some(10));
        assert_eq!(ctrl.get_column_memory(0), Some(10));
    }

    // -- camelCase word movement --------------------------------------------

    #[test]
    fn word_right_camel_case() {
        let model = SimpleModel::new("camelCaseWord");
        let c = cursor_at(1, 1);
        let r = move_word_right(&model, &c, false);
        // Should stop at end of "camel" (before "C")
        assert_eq!(r.position(), Position::new(1, 6));
        let r2 = move_word_right(&model, &r, false);
        // Should stop at end of "Case"
        assert_eq!(r2.position(), Position::new(1, 10));
        let r3 = move_word_right(&model, &r2, false);
        // Should stop at end of "Word"
        assert_eq!(r3.position(), Position::new(1, 14));
    }

    #[test]
    fn word_left_camel_case() {
        let model = SimpleModel::new("camelCaseWord");
        let c = cursor_at(1, 14);
        let r = move_word_left(&model, &c, false);
        assert_eq!(r.position(), Position::new(1, 10)); // start of "Word"
        let r2 = move_word_left(&model, &r, false);
        assert_eq!(r2.position(), Position::new(1, 6)); // start of "Case"
        let r3 = move_word_left(&model, &r2, false);
        assert_eq!(r3.position(), Position::new(1, 1)); // start of "camel"
    }

    #[test]
    fn word_right_underscore_boundaries() {
        let model = SimpleModel::new("snake_case_word");
        let c = cursor_at(1, 1);
        let r = move_word_right(&model, &c, false);
        assert_eq!(r.position(), Position::new(1, 6)); // end of "snake"
        let r2 = move_word_right(&model, &r, false);
        assert_eq!(r2.position(), Position::new(1, 7)); // past "_"
        let r3 = move_word_right(&model, &r2, false);
        assert_eq!(r3.position(), Position::new(1, 11)); // end of "case"
    }

    #[test]
    fn word_left_underscore_boundaries() {
        let model = SimpleModel::new("snake_case_word");
        let c = cursor_at(1, 16);
        let r = move_word_left(&model, &c, false);
        assert_eq!(r.position(), Position::new(1, 12)); // start of "word"
        let r2 = move_word_left(&model, &r, false);
        assert_eq!(r2.position(), Position::new(1, 11)); // start of "_"
    }

    #[test]
    fn word_right_all_caps_then_lowercase() {
        // "HTMLParser" — should stop: HTML|Parser
        let model = SimpleModel::new("HTMLParser");
        let c = cursor_at(1, 1);
        let r = move_word_right(&model, &c, false);
        // Should stop at boundary between HTML and Parser
        assert_eq!(r.position(), Position::new(1, 5)); // end of "HTML"
        let r2 = move_word_right(&model, &r, false);
        assert_eq!(r2.position(), Position::new(1, 11)); // end of "Parser"
    }

    // -- delete word boundaries ---------------------------------------------

    #[test]
    fn delete_word_left_returns_range() {
        let model = SimpleModel::new("hello world");
        let c = cursor_at(1, 12);
        let (start, end) = delete_word_left(&model, &c);
        assert_eq!(start, Position::new(1, 7));
        assert_eq!(end, Position::new(1, 12));
    }

    #[test]
    fn delete_word_right_returns_range() {
        let model = SimpleModel::new("hello world");
        let c = cursor_at(1, 1);
        let (start, end) = delete_word_right(&model, &c);
        assert_eq!(start, Position::new(1, 1));
        assert_eq!(end, Position::new(1, 6));
    }

    // -- select_word_at (double-click) --------------------------------------

    #[test]
    fn select_word_at_basic() {
        let model = SimpleModel::new("hello world");
        let c = cursor_at(1, 3); // in "hello"
        let r = select_word_at(&model, &c);
        assert_eq!(r.selection.anchor, Position::new(1, 1));
        assert_eq!(r.selection.active, Position::new(1, 6));
    }

    #[test]
    fn select_word_at_separator() {
        let model = SimpleModel::new("foo..bar");
        let c = cursor_at(1, 5); // on second "."
        let r = select_word_at(&model, &c);
        assert_eq!(r.selection.anchor, Position::new(1, 4));
        assert_eq!(r.selection.active, Position::new(1, 6));
    }

    #[test]
    fn select_word_at_whitespace() {
        let model = SimpleModel::new("hello   world");
        let c = cursor_at(1, 7); // in whitespace
        let r = select_word_at(&model, &c);
        assert_eq!(r.selection.anchor, Position::new(1, 6));
        assert_eq!(r.selection.active, Position::new(1, 9));
    }

    #[test]
    fn select_word_at_with_underscore() {
        let model = SimpleModel::new("my_var = 1");
        let c = cursor_at(1, 4); // on "v" in "my_var"
        let r = select_word_at(&model, &c);
        assert_eq!(r.selection.anchor, Position::new(1, 1));
        assert_eq!(r.selection.active, Position::new(1, 7));
    }
}
