//! Multi-cursor controller for vsedit.
//!
//! Equivalent to VS Code's `vs/editor/common/cursorCommon.ts` and related files.
//! Provides [`CursorState`], [`CursorController`], and standalone cursor movement
//! functions that operate on a single cursor against an [`ITextModel`].

use std::fmt;

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

    /// Returns `true` when the cursor has a non-empty selection (anchor ≠ active).
    pub fn is_selection(&self) -> bool {
        self.selection.anchor != self.selection.active
    }

    /// Count the number of lines spanned by the selection.
    ///
    /// A collapsed cursor returns 0. A selection within a single line returns 1.
    /// A selection spanning from line 2 to line 5 returns 4.
    pub fn selection_line_count(&self) -> u32 {
        if !self.is_selection() {
            return 0;
        }
        let a = self.selection.anchor.line;
        let b = self.selection.active.line;
        if a > b { a - b + 1 } else { b - a + 1 }
    }
}

impl fmt::Display for CursorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pos = self.position();
        write!(f, "Ln {}, Col {}", pos.line, pos.column)
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

    /// Return the number of active cursors.
    pub fn cursor_count(&self) -> usize {
        self.cursors.len()
    }

    /// Return the positions of all cursors.
    pub fn positions(&self) -> Vec<Position> {
        self.cursors.iter().map(|c| c.position()).collect()
    }

    /// Return `true` when the primary cursor is at line 1, column 1.
    pub fn is_at_origin(&self) -> bool {
        let pos = self.cursors[0].position();
        pos.line == 1 && pos.column == 1
    }

    /// Remove all secondary cursors, keeping only the primary cursor.
    /// This is an alias for [`remove_secondary_cursors`](Self::remove_secondary_cursors).
    pub fn clear_secondary(&mut self) {
        self.remove_secondary_cursors();
    }
}

impl Default for CursorController {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CursorController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.cursors.len();
        if n == 1 {
            write!(f, "1 cursor")
        } else {
            write!(f, "{n} cursors")
        }
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
// Cursor sorting
// ---------------------------------------------------------------------------

/// Sort cursors by position (line first, then column).
pub fn sort_cursors(cursors: &mut [CursorState]) {
    cursors.sort_by(|a, b| {
        let pa = a.position();
        let pb = b.position();
        pa.line.cmp(&pb.line).then(pa.column.cmp(&pb.column))
    });
}

// ---------------------------------------------------------------------------
// Cursor column alignment
// ---------------------------------------------------------------------------

/// Align all cursors to a specific column.
pub fn align_cursors_to_column(cursors: &mut [CursorState], column: u32) {
    for cursor in cursors.iter_mut() {
        let pos = cursor.position();
        let new_pos = Position::new(pos.line, column);
        *cursor = CursorState::from_position(new_pos);
    }
}

/// Align all cursors to the maximum column among them.
pub fn align_cursors_to_max_column(cursors: &mut [CursorState]) {
    let max_col = cursors.iter().map(|c| c.position().column).max().unwrap_or(1);
    align_cursors_to_column(cursors, max_col);
}

// ---------------------------------------------------------------------------
// Cursor serialization
// ---------------------------------------------------------------------------

/// Serialize cursor positions to a compact string format.
///
/// Each cursor is represented as `line:column` separated by semicolons.
pub fn serialize_cursors(cursors: &[CursorState]) -> String {
    cursors
        .iter()
        .map(|c| {
            let p = c.position();
            format!("{}:{}", p.line, p.column)
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Deserialize cursor positions from the compact string format.
///
/// Returns `None` if the input is malformed.
pub fn deserialize_cursors(input: &str) -> Option<Vec<CursorState>> {
    if input.is_empty() {
        return Some(Vec::new());
    }
    let mut cursors = Vec::new();
    for part in input.split(';') {
        let mut parts = part.split(':');
        let line: u32 = parts.next()?.parse().ok()?;
        let column: u32 = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        cursors.push(CursorState::from_position(Position::new(line, column)));
    }
    Some(cursors)
}

// ---------------------------------------------------------------------------
// Cursor movement patterns
// ---------------------------------------------------------------------------

/// Move a cursor to the beginning of the current word, or to the previous
/// word boundary if already at one.
pub fn move_to_word_start(cursor: &CursorState, model: &dyn ITextModel) -> CursorState {
    let pos = cursor.position();
    let line_content = model.get_line_content(pos.line);
    let col_idx = (pos.column as usize).saturating_sub(1);
    let bytes = line_content.as_bytes();

    // Skip whitespace going left
    let mut i = col_idx;
    while i > 0 && bytes.get(i - 1).map_or(false, |b| b.is_ascii_whitespace()) {
        i -= 1;
    }
    // Skip word chars going left
    while i > 0 && bytes.get(i - 1).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_') {
        i -= 1;
    }

    CursorState::from_position(Position::new(pos.line, (i as u32) + 1))
}

/// Move a cursor to the end of the current word, or to the next
/// word boundary if already at one.
pub fn move_to_word_end(cursor: &CursorState, model: &dyn ITextModel) -> CursorState {
    let pos = cursor.position();
    let line_content = model.get_line_content(pos.line);
    let col_idx = (pos.column as usize).saturating_sub(1);
    let len = line_content.len();
    let bytes = line_content.as_bytes();

    let mut i = col_idx;
    // Skip word chars going right
    while i < len && bytes.get(i).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_') {
        i += 1;
    }
    // Skip whitespace going right
    while i < len && bytes.get(i).map_or(false, |b| b.is_ascii_whitespace()) {
        i += 1;
    }

    CursorState::from_position(Position::new(pos.line, (i as u32) + 1))
}

/// Check if two cursor states overlap (same position or intersecting selections).
pub fn cursors_overlap(a: &CursorState, b: &CursorState) -> bool {
    if !a.is_selection() && !b.is_selection() {
        return a.position() == b.position();
    }
    let a_start = std::cmp::min(a.selection.anchor, a.selection.active);
    let a_end = std::cmp::max(a.selection.anchor, a.selection.active);
    let b_start = std::cmp::min(b.selection.anchor, b.selection.active);
    let b_end = std::cmp::max(b.selection.anchor, b.selection.active);
    a_start <= b_end && b_start <= a_end
}

/// Compute a summary of cursor positions.
#[derive(Debug, Clone)]
pub struct CursorSummary {
    pub count: usize,
    pub min_line: u32,
    pub max_line: u32,
    pub lines_with_cursors: usize,
}

/// Summarize the cursor positions in a controller.
pub fn cursor_summary(ctrl: &CursorController) -> CursorSummary {
    let cursors = ctrl.get_all();
    let mut lines = std::collections::BTreeSet::new();
    let mut min_line = u32::MAX;
    let mut max_line = 0;
    for c in cursors {
        let l = c.position().line;
        lines.insert(l);
        min_line = min_line.min(l);
        max_line = max_line.max(l);
    }
    CursorSummary {
        count: cursors.len(),
        min_line,
        max_line,
        lines_with_cursors: lines.len(),
    }
}

// ---------------------------------------------------------------------------
// Cursor distance and grouping utilities
// ---------------------------------------------------------------------------

/// Compute the Manhattan distance between two cursor positions.
pub fn cursor_distance(a: &CursorState, b: &CursorState) -> u64 {
    let pa = a.position();
    let pb = b.position();
    let line_diff = (pa.line as i64 - pb.line as i64).unsigned_abs();
    let col_diff = (pa.column as i64 - pb.column as i64).unsigned_abs();
    line_diff + col_diff
}

/// Find the cursor nearest to a given position among a slice of cursors.
/// Returns `None` if the slice is empty.
pub fn nearest_cursor(cursors: &[CursorState], target: Position) -> Option<usize> {
    if cursors.is_empty() {
        return None;
    }
    let target_state = CursorState::from_position(target);
    let mut best_idx = 0;
    let mut best_dist = cursor_distance(&cursors[0], &target_state);
    for (i, c) in cursors.iter().enumerate().skip(1) {
        let d = cursor_distance(c, &target_state);
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    Some(best_idx)
}

/// Group cursors by their line number, returning a map from line to cursor indices.
pub fn group_cursors_by_line(cursors: &[CursorState]) -> std::collections::BTreeMap<u32, Vec<usize>> {
    let mut map = std::collections::BTreeMap::new();
    for (i, c) in cursors.iter().enumerate() {
        map.entry(c.position().line).or_insert_with(Vec::new).push(i);
    }
    map
}

/// Return only cursors that lie within the given line range (inclusive).
pub fn filter_cursors_in_range(cursors: &[CursorState], start_line: u32, end_line: u32) -> Vec<CursorState> {
    cursors
        .iter()
        .filter(|c| {
            let line = c.position().line;
            line >= start_line && line <= end_line
        })
        .cloned()
        .collect()
}

/// Check if all cursors are on the same line.
pub fn all_cursors_same_line(cursors: &[CursorState]) -> bool {
    if cursors.len() <= 1 {
        return true;
    }
    let first_line = cursors[0].position().line;
    cursors.iter().all(|c| c.position().line == first_line)
}

/// Return the line span (max_line - min_line + 1) covered by the given cursors.
pub fn cursor_line_span(cursors: &[CursorState]) -> u32 {
    if cursors.is_empty() {
        return 0;
    }
    let min = cursors.iter().map(|c| c.position().line).min().unwrap();
    let max = cursors.iter().map(|c| c.position().line).max().unwrap();
    max - min + 1
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// Cursor deduplication and bounding box utilities
// ---------------------------------------------------------------------------

/// Remove duplicate cursors (same position), keeping the first occurrence.
pub fn deduplicate_cursors(cursors: &[CursorState]) -> Vec<CursorState> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for c in cursors {
        let key = (
            c.selection.anchor.line,
            c.selection.anchor.column,
            c.selection.active.line,
            c.selection.active.column,
        );
        if seen.insert(key) {
            result.push(c.clone());
        }
    }
    result
}

/// Compute the bounding box (top-left, bottom-right) of all cursor positions.
/// Returns `None` if the slice is empty.
pub fn cursor_bounding_box(cursors: &[CursorState]) -> Option<(Position, Position)> {
    if cursors.is_empty() {
        return None;
    }
    let mut min_line = u32::MAX;
    let mut min_col = u32::MAX;
    let mut max_line = 0u32;
    let mut max_col = 0u32;
    for c in cursors {
        let pos = c.position();
        min_line = min_line.min(pos.line);
        min_col = min_col.min(pos.column);
        max_line = max_line.max(pos.line);
        max_col = max_col.max(pos.column);
    }
    Some((Position::new(min_line, min_col), Position::new(max_line, max_col)))
}

/// Return the cursor closest to the end of the document.
pub fn last_cursor(cursors: &[CursorState]) -> Option<&CursorState> {
    cursors
        .iter()
        .max_by_key(|c| (c.position().line, c.position().column))
}

/// Return the cursor closest to the start of the document.
pub fn first_cursor(cursors: &[CursorState]) -> Option<&CursorState> {
    cursors
        .iter()
        .min_by_key(|c| (c.position().line, c.position().column))
}

/// Check if any cursor has a non-empty selection.
pub fn any_has_selection(cursors: &[CursorState]) -> bool {
    cursors.iter().any(|c| c.is_selection())
}

/// Count how many cursors have active selections.
pub fn selection_count(cursors: &[CursorState]) -> usize {
    cursors.iter().filter(|c| c.is_selection()).count()
}

/// Return cursors on a specific line.
pub fn cursors_on_line(cursors: &[CursorState], line: u32) -> Vec<&CursorState> {
    cursors
        .iter()
        .filter(|c| c.position().line == line)
        .collect()
}

/// Reverse the order of cursors in a mutable slice.
pub fn reverse_cursors(cursors: &mut [CursorState]) {
    cursors.reverse();
}

/// Collapse all selections to their active position (no selection, just caret).
pub fn collapse_selections(cursors: &[CursorState]) -> Vec<CursorState> {
    cursors
        .iter()
        .map(|c| CursorState::from_position(c.position()))
        .collect()
}

impl CursorState {
    /// Create a cursor with a selection from anchor to active positions.
    pub fn with_selection(anchor: Position, active: Position) -> Self {
        Self {
            selection: Selection::from_positions(anchor, active),
        }
    }

    /// Return the anchor position of the selection.
    pub fn anchor(&self) -> Position {
        self.selection.anchor
    }

    /// Return a collapsed version of this cursor (selection removed).
    pub fn collapsed(&self) -> Self {
        CursorState::from_position(self.position())
    }
}

// ---------------------------------------------------------------------------
// CursorSoftWrapHandler – visual vs logical line positions
// ---------------------------------------------------------------------------

/// Handles cursor movement in soft-wrapped lines by tracking the mapping
/// between visual (wrapped) line positions and logical (buffer) line positions.
#[derive(Clone, Debug)]
pub struct CursorSoftWrapHandler {
    /// The wrap width in columns (e.g. 80).
    wrap_width: u32,
}

impl CursorSoftWrapHandler {
    /// Create a new handler with the given wrap width.
    pub fn new(wrap_width: u32) -> Self {
        assert!(wrap_width > 0, "wrap width must be positive");
        Self { wrap_width }
    }

    /// Return the wrap width.
    pub fn wrap_width(&self) -> u32 {
        self.wrap_width
    }

    /// Given a logical column (1-based), return the visual line offset (0-based)
    /// and visual column (1-based) within that visual line.
    pub fn logical_to_visual(&self, logical_col: u32) -> (u32, u32) {
        if logical_col == 0 {
            return (0, 1);
        }
        let zero_col = logical_col - 1;
        let visual_line = zero_col / self.wrap_width;
        let visual_col = (zero_col % self.wrap_width) + 1;
        (visual_line, visual_col)
    }

    /// Convert a visual line offset and visual column back to a logical column.
    pub fn visual_to_logical(&self, visual_line: u32, visual_col: u32) -> u32 {
        visual_line * self.wrap_width + visual_col
    }

    /// How many visual lines does a logical line of the given length occupy?
    pub fn visual_line_count(&self, line_length: u32) -> u32 {
        if line_length == 0 {
            return 1;
        }
        ((line_length - 1) / self.wrap_width) + 1
    }

    /// Compute the visual position for a cursor inside a model.
    /// Returns `(visual_line_offset, visual_column)` relative to the start
    /// of the logical line.
    pub fn cursor_visual_position(&self, cursor: &CursorState) -> (u32, u32) {
        self.logical_to_visual(cursor.position().column)
    }
}

// ---------------------------------------------------------------------------
// CursorColumnMemory – sticky column across vertical moves
// ---------------------------------------------------------------------------

/// Preserves the desired column position across vertical cursor moves so that
/// moving through shorter lines and back to a longer line restores the
/// original column.
#[derive(Clone, Debug)]
pub struct CursorColumnMemory {
    desired_column: Option<u32>,
}

impl CursorColumnMemory {
    pub fn new() -> Self {
        Self { desired_column: None }
    }

    /// Record the desired column (call on horizontal moves).
    pub fn set(&mut self, column: u32) {
        self.desired_column = Some(column);
    }

    /// Clear the memory (call on explicit horizontal repositioning).
    pub fn clear(&mut self) {
        self.desired_column = None;
    }

    /// Return the memorised column, if any.
    pub fn get(&self) -> Option<u32> {
        self.desired_column
    }

    /// Resolve the effective column for a vertical move: use the memorised
    /// column if set, otherwise the cursor's current column, clamped to
    /// `max_column`.
    pub fn resolve(&self, current_col: u32, max_column: u32) -> u32 {
        let desired = self.desired_column.unwrap_or(current_col);
        desired.min(max_column)
    }

    /// Perform a vertical move: set memory from current position if not
    /// already set, then return the clamped column for the target line.
    pub fn apply_vertical_move(&mut self, current_col: u32, target_max_col: u32) -> u32 {
        if self.desired_column.is_none() {
            self.desired_column = Some(current_col);
        }
        self.resolve(current_col, target_max_col)
    }
}

impl Default for CursorColumnMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CursorWordBoundary – configurable word-boundary rules
// ---------------------------------------------------------------------------

/// Language-specific word boundary detection with configurable separator sets.
#[derive(Clone, Debug)]
pub struct CursorWordBoundary {
    /// Extra characters that are treated as word separators in addition to
    /// the default whitespace and punctuation rules.
    extra_separators: Vec<u8>,
    /// When true, camelCase transitions count as word boundaries.
    camel_case: bool,
}

impl CursorWordBoundary {
    /// Create a boundary detector with default rules (camelCase enabled,
    /// no extra separators).
    pub fn new() -> Self {
        Self {
            extra_separators: Vec::new(),
            camel_case: true,
        }
    }

    /// Enable or disable camelCase boundary detection.
    pub fn set_camel_case(&mut self, enabled: bool) {
        self.camel_case = enabled;
    }

    /// Add extra separator characters (e.g. `-`, `.` for CSS/HTML).
    pub fn add_separators(&mut self, seps: &[u8]) {
        for &b in seps {
            if !self.extra_separators.contains(&b) {
                self.extra_separators.push(b);
            }
        }
    }

    /// Classify a byte taking extra separators into account.
    fn classify(&self, ch: u8) -> CharClass {
        if self.extra_separators.contains(&ch) {
            return CharClass::Separator;
        }
        classify_char(ch)
    }

    /// Returns true if there is a word boundary between the two bytes.
    pub fn is_boundary(&self, left: u8, right: u8) -> bool {
        let lc = self.classify(left);
        let rc = self.classify(right);
        if lc == rc {
            return false;
        }
        if self.camel_case && lc == CharClass::Lowercase && rc == CharClass::Uppercase {
            return true;
        }
        if lc == CharClass::Underscore && rc == CharClass::Underscore {
            return false;
        }
        lc != rc
    }

    /// Find the column of the previous word boundary on a line (1-based),
    /// starting from `col` (1-based, exclusive). Returns 1 if no boundary
    /// is found.
    pub fn find_prev_boundary(&self, line: &str, col: u32) -> u32 {
        let bytes = line.as_bytes();
        let start = ((col as usize).min(bytes.len())).saturating_sub(1);
        if start == 0 {
            return 1;
        }
        // skip initial whitespace
        let mut i = start;
        while i > 0 && classify_char(bytes[i]) == CharClass::Whitespace {
            i -= 1;
        }
        while i > 0 {
            if self.is_boundary(bytes[i - 1], bytes[i]) {
                return (i as u32) + 1;
            }
            i -= 1;
        }
        1
    }

    /// Find the column *after* the next word boundary on a line (1-based).
    /// Returns `line.len() + 1` if no boundary is found.
    pub fn find_next_boundary(&self, line: &str, col: u32) -> u32 {
        let bytes = line.as_bytes();
        let start = (col as usize).min(bytes.len());
        if start >= bytes.len() {
            return (bytes.len() as u32) + 1;
        }
        let mut i = start;
        // skip initial whitespace
        while i < bytes.len() && classify_char(bytes[i]) == CharClass::Whitespace {
            i += 1;
        }
        while i < bytes.len() {
            if i > start && self.is_boundary(bytes[i - 1], bytes[i]) {
                return (i as u32) + 1;
            }
            i += 1;
        }
        (bytes.len() as u32) + 1
    }
}

impl Default for CursorWordBoundary {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CursorBlinkTimer – blink state management
// ---------------------------------------------------------------------------

/// Manages cursor blink state with a configurable interval.
#[derive(Clone, Debug)]
pub struct CursorBlinkTimer {
    /// Interval in milliseconds between blink toggles.
    interval_ms: u64,
    /// Accumulated time in milliseconds since the last toggle.
    elapsed_ms: u64,
    /// Whether the cursor is currently visible.
    visible: bool,
    /// Whether blinking is enabled at all.
    enabled: bool,
}

impl CursorBlinkTimer {
    /// Create a new blink timer with the given interval in milliseconds.
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            elapsed_ms: 0,
            visible: true,
            enabled: true,
        }
    }

    /// Advance the timer by `delta_ms` milliseconds.
    /// Returns `true` if the visibility state changed.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.enabled {
            return false;
        }
        self.elapsed_ms += delta_ms;
        let mut changed = false;
        while self.elapsed_ms >= self.interval_ms {
            self.elapsed_ms -= self.interval_ms;
            self.visible = !self.visible;
            changed = true;
        }
        changed
    }

    /// Returns `true` if the cursor should be drawn.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Reset the timer so the cursor is immediately visible.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
        self.visible = true;
    }

    /// Enable or disable blinking. When disabled the cursor is always visible.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.visible = true;
            self.elapsed_ms = 0;
        }
    }

    /// Returns `true` if blinking is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Return the current interval in milliseconds.
    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    /// Update the blink interval. Resets accumulated time.
    pub fn set_interval_ms(&mut self, ms: u64) {
        self.interval_ms = ms;
        self.elapsed_ms = 0;
    }
}


// ---------------------------------------------------------------------------
// CursorViewportScroller - cursor viewport scroller
// ---------------------------------------------------------------------------

/// Severity level for cursor viewport scroller issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CursorViewportScrollerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for CursorViewportScrollerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [CursorViewportScroller].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorViewportScrollerEntry {
    pub id: String,
    pub label: String,
    pub severity: CursorViewportScrollerSeverity,
    pub detail: Option<String>,
    pub viewport_lines: usize,
    enabled: bool,
}

impl CursorViewportScrollerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: CursorViewportScrollerSeverity::Low,
            detail: None,
            viewport_lines: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: CursorViewportScrollerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_viewport_lines(mut self, val: usize) -> Self {
        self.viewport_lines = val;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.enabled && self.severity >= CursorViewportScrollerSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.viewport_lines, det)
    }
}

impl fmt::Display for CursorViewportScrollerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [CursorViewportScrollerEntry] items.
#[derive(Debug, Clone)]
pub struct CursorViewportScroller {
    entries: Vec<CursorViewportScrollerEntry>,
    name: String,
    capacity: usize,
}

impl CursorViewportScroller {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: CursorViewportScrollerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<CursorViewportScrollerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&CursorViewportScrollerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn viewport_lines(&self) -> usize { self.entries.len() }

    pub fn is_visible(&self) -> bool {
        self.entries.iter().any(|e| e.is_visible())
    }

    pub fn entries_by_severity(&self, severity: CursorViewportScrollerSeverity) -> Vec<&CursorViewportScrollerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= CursorViewportScrollerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&CursorViewportScrollerEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&CursorViewportScrollerEntry> {
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
// CursorSelectionExpander - cursor selection expander
// ---------------------------------------------------------------------------

/// Configuration for [CursorSelectionExpander].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorSelectionExpanderConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub selection_length: usize,
}

impl CursorSelectionExpanderConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, selection_length: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_selection_length(mut self, val: usize) -> Self { self.selection_length = val; self }
}

impl Default for CursorSelectionExpanderConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [CursorSelectionExpander].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorSelectionExpanderItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl CursorSelectionExpanderItem {
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

    pub fn has_selection(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for CursorSelectionExpanderItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [CursorSelectionExpanderItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct CursorSelectionExpander {
    config: CursorSelectionExpanderConfig,
    items: Vec<CursorSelectionExpanderItem>,
}

impl CursorSelectionExpander {
    pub fn new(config: CursorSelectionExpanderConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: CursorSelectionExpanderItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<CursorSelectionExpanderItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&CursorSelectionExpanderItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn selection_length(&self) -> usize { self.items.len() }

    pub fn has_selection(&self) -> bool {
        self.items.iter().any(|i| i.has_selection())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&CursorSelectionExpanderItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&CursorSelectionExpanderItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &CursorSelectionExpanderConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Cursor configuration manager.
#[derive(Debug, Clone)]
pub struct CursorConfig {
    entries: Vec<CursorEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single cursor entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl CursorEntry {
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

impl CursorConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: CursorEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&CursorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut CursorEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&CursorEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&CursorEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&CursorEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<CursorEntry> {
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
// xa_ extended helpers for cursor
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaCursorRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaCursorRingBuf {
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
pub struct XaCursorCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaCursorCounter {
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

impl Default for XaCursorCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 24
// ---------------------------------------------------------------------------

/// Generic object pool `Xc24Pool<T>`.
pub struct Xc24Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc24Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc24PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc24Pool<T> {
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
    pub fn stats(&self) -> Xc24PoolStats {
        Xc24PoolStats {
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

impl<T> Default for Xc24Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc24Scheduler`.
pub struct Xc24Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc24Scheduler {
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

impl Default for Xc24Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_24 hash for the given byte slice.
pub fn xc_24_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_24 convention.
pub fn xc_24_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_65 deepening: state machine + event bus ---

/// States for the Xd65 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd65State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd65State {
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
pub struct Xd65Transition {
    pub from: Xd65State,
    pub to: Xd65State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd65StateMachine {
    current: Xd65State,
    history: Vec<Xd65Transition>,
    step_counter: usize,
}

impl Xd65StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd65State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd65State {
        self.current
    }

    pub fn history(&self) -> &[Xd65Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd65State) -> Result<Xd65State, String> {
        let allowed = match (self.current, target) {
            (Xd65State::Idle, Xd65State::Running) => true,
            (Xd65State::Running, Xd65State::Paused) => true,
            (Xd65State::Running, Xd65State::Done) => true,
            (Xd65State::Paused, Xd65State::Running) => true,
            (Xd65State::Paused, Xd65State::Done) => true,
            (Xd65State::Done, Xd65State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_65: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd65Transition {
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
            "Xd65SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd65State> {
        let prefix = "Xd65SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd65State::Idle),
            "Running" => Some(Xd65State::Running),
            "Paused" => Some(Xd65State::Paused),
            "Done" => Some(Xd65State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd65State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd65 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd65Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd65Event {
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

type Xd65HandlerFn = Box<dyn Fn(&Xd65Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd65EventBus {
    handlers: Vec<(usize, Option<String>, Xd65HandlerFn)>,
    next_id: usize,
    published: Vec<Xd65Event>,
}

impl Xd65EventBus {
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
        F: Fn(&Xd65Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd65Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd65Event) {
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

    pub fn published_events(&self) -> &[Xd65Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #66
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf66Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf66TrieNode {
    children: std::collections::HashMap<char, Xf66TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf66Trie {
    root: Xf66TrieNode,
    count: usize,
}

impl Xf66Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf66TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf66TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf66TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf66BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf66BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 23).
pub struct Xh23SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh23SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 65 as u64,
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

/// A compact bit set supporting boolean operations (variant 23).
pub struct Xh23BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh23BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 23).
pub struct Xi23Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi23Deque<T> {
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
pub struct Xi23Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi23Interval {
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

/// A simple interval tree (variant 23).
pub struct Xi23IntervalTree {
    xi_intervals: Vec<Xi23Interval>,
}

impl Xi23IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi23Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi23Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi23Interval) -> Vec<&Xi23Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi23Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi23Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi23Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi23Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi23Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi23Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 23) ---

/// Disjoint set / union-find for crate 23.
pub struct Xj23UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj23UnionFind {
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

const XJ23_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 23.
pub struct Xj23BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj23BTreeNode<K, V>>>,
    len: usize,
}

struct Xj23BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj23BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj23BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ23_BTREE_ORDER - 1
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
        let mid = XJ23_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj23BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj23BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj23BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj23BTreeNode::xj_new_leaf();
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

    // -- New method tests ---------------------------------------------------

    #[test]
    fn cursor_count_single() {
        let ctrl = CursorController::new();
        assert_eq!(ctrl.cursor_count(), 1);
    }

    #[test]
    fn cursor_count_multiple() {
        let mut ctrl = CursorController::new();
        ctrl.add_cursor(Position::new(2, 1));
        ctrl.add_cursor(Position::new(3, 1));
        assert_eq!(ctrl.cursor_count(), 3);
    }

    #[test]
    fn positions_returns_all() {
        let mut ctrl = CursorController::from_position(Position::new(1, 5));
        ctrl.add_cursor(Position::new(3, 2));
        let positions = ctrl.positions();
        assert_eq!(positions, vec![Position::new(1, 5), Position::new(3, 2)]);
    }

    #[test]
    fn is_at_origin_true() {
        let ctrl = CursorController::new(); // default is (1,1)
        assert!(ctrl.is_at_origin());
    }

    #[test]
    fn is_at_origin_false() {
        let ctrl = CursorController::from_position(Position::new(2, 3));
        assert!(!ctrl.is_at_origin());
    }

    #[test]
    fn clear_secondary_removes_extra_cursors() {
        let mut ctrl = CursorController::new();
        ctrl.add_cursor(Position::new(2, 1));
        ctrl.add_cursor(Position::new(3, 1));
        assert_eq!(ctrl.cursor_count(), 3);
        ctrl.clear_secondary();
        assert_eq!(ctrl.cursor_count(), 1);
        assert!(ctrl.is_at_origin());
    }

    #[test]
    fn is_selection_collapsed() {
        let c = cursor_at(1, 1);
        assert!(!c.is_selection());
    }

    #[test]
    fn is_selection_with_range() {
        let c = CursorState {
            selection: Selection::new(1, 1, 1, 5),
        };
        assert!(c.is_selection());
    }

    #[test]
    fn selection_line_count_collapsed() {
        let c = cursor_at(3, 4);
        assert_eq!(c.selection_line_count(), 0);
    }

    #[test]
    fn selection_line_count_single_line() {
        let c = CursorState {
            selection: Selection::new(2, 1, 2, 8),
        };
        assert_eq!(c.selection_line_count(), 1);
    }

    #[test]
    fn selection_line_count_multi_line() {
        let c = CursorState {
            selection: Selection::new(2, 1, 5, 3),
        };
        assert_eq!(c.selection_line_count(), 4);
    }

    #[test]
    fn selection_line_count_reverse() {
        // anchor is after active (backwards selection)
        let c = CursorState {
            selection: Selection::new(5, 3, 2, 1),
        };
        assert_eq!(c.selection_line_count(), 4);
    }

    #[test]
    fn display_cursor_state() {
        let c = cursor_at(10, 42);
        assert_eq!(format!("{c}"), "Ln 10, Col 42");
    }

    #[test]
    fn display_cursor_controller_single() {
        let ctrl = CursorController::new();
        assert_eq!(format!("{ctrl}"), "1 cursor");
    }

    #[test]
    fn display_cursor_controller_multiple() {
        let mut ctrl = CursorController::new();
        ctrl.add_cursor(Position::new(2, 1));
        ctrl.add_cursor(Position::new(3, 1));
        assert_eq!(format!("{ctrl}"), "3 cursors");
    }

    // -- new tests --

    #[test]
    fn sort_cursors_orders_by_line_then_column() {
        let mut cursors = vec![
            cursor_at(5, 3),
            cursor_at(1, 10),
            cursor_at(1, 1),
            cursor_at(3, 5),
        ];
        sort_cursors(&mut cursors);
        assert_eq!(cursors[0].position(), Position::new(1, 1));
        assert_eq!(cursors[1].position(), Position::new(1, 10));
        assert_eq!(cursors[2].position(), Position::new(3, 5));
        assert_eq!(cursors[3].position(), Position::new(5, 3));
    }

    #[test]
    fn align_cursors_to_column_sets_all() {
        let mut cursors = vec![cursor_at(1, 5), cursor_at(2, 10), cursor_at(3, 1)];
        align_cursors_to_column(&mut cursors, 7);
        for c in &cursors {
            assert_eq!(c.position().column, 7);
        }
    }

    #[test]
    fn align_cursors_to_max_column_uses_max() {
        let mut cursors = vec![cursor_at(1, 5), cursor_at(2, 10), cursor_at(3, 1)];
        align_cursors_to_max_column(&mut cursors);
        for c in &cursors {
            assert_eq!(c.position().column, 10);
        }
    }

    #[test]
    fn serialize_and_deserialize_roundtrip() {
        let cursors = vec![cursor_at(1, 1), cursor_at(5, 10), cursor_at(100, 42)];
        let serialized = serialize_cursors(&cursors);
        assert_eq!(serialized, "1:1;5:10;100:42");
        let deserialized = deserialize_cursors(&serialized).unwrap();
        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized[0].position(), Position::new(1, 1));
        assert_eq!(deserialized[2].position(), Position::new(100, 42));
    }

    #[test]
    fn deserialize_cursors_empty_string() {
        let result = deserialize_cursors("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn deserialize_cursors_malformed_returns_none() {
        assert!(deserialize_cursors("abc").is_none());
        assert!(deserialize_cursors("1:2:3").is_none());
    }

    #[test]
    fn move_to_word_start_from_middle() {
        let model = SimpleModel::new("hello world");
        let cursor = cursor_at(1, 8); // in the middle of "world"
        let result = move_to_word_start(&cursor, &model);
        assert_eq!(result.position(), Position::new(1, 7)); // start of "world"
    }

    #[test]
    fn move_to_word_end_from_middle() {
        let model = SimpleModel::new("hello world");
        let cursor = cursor_at(1, 2); // in the middle of "hello"
        let result = move_to_word_end(&cursor, &model);
        // Should move past "ello" and the space to "world" start
        assert_eq!(result.position().line, 1);
        assert!(result.position().column > 2);
    }

    #[test]
    fn cursors_overlap_same_position() {
        assert!(cursors_overlap(&cursor_at(1, 1), &cursor_at(1, 1)));
    }

    #[test]
    fn cursors_overlap_different_positions() {
        assert!(!cursors_overlap(&cursor_at(1, 1), &cursor_at(2, 1)));
    }

    #[test]
    fn cursor_summary_basic() {
        let mut ctrl = CursorController::new();
        ctrl.add_cursor(Position::new(5, 1));
        ctrl.add_cursor(Position::new(10, 1));
        let s = cursor_summary(&ctrl);
        assert_eq!(s.count, 3);
        assert_eq!(s.min_line, 1);
        assert_eq!(s.max_line, 10);
        assert_eq!(s.lines_with_cursors, 3);
    }

    #[test]
    fn cursor_distance_same_position() {
        let a = cursor_at(1, 1);
        let b = cursor_at(1, 1);
        assert_eq!(cursor_distance(&a, &b), 0);
    }

    #[test]
    fn cursor_distance_different_lines() {
        let a = cursor_at(1, 1);
        let b = cursor_at(5, 3);
        assert_eq!(cursor_distance(&a, &b), 6); // 4 lines + 2 cols
    }

    #[test]
    fn nearest_cursor_finds_closest() {
        let cursors = vec![cursor_at(1, 1), cursor_at(10, 5), cursor_at(3, 2)];
        let idx = nearest_cursor(&cursors, Position::new(3, 3)).unwrap();
        assert_eq!(idx, 2);
    }

    #[test]
    fn nearest_cursor_empty_returns_none() {
        assert!(nearest_cursor(&[], Position::new(1, 1)).is_none());
    }

    #[test]
    fn group_cursors_by_line_groups_correctly() {
        let cursors = vec![cursor_at(1, 1), cursor_at(1, 5), cursor_at(3, 2)];
        let groups = group_cursors_by_line(&cursors);
        assert_eq!(groups[&1].len(), 2);
        assert_eq!(groups[&3].len(), 1);
    }

    #[test]
    fn filter_cursors_in_range_filters() {
        let cursors = vec![cursor_at(1, 1), cursor_at(5, 1), cursor_at(10, 1)];
        let filtered = filter_cursors_in_range(&cursors, 3, 7);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].position().line, 5);
    }

    #[test]
    fn all_cursors_same_line_true() {
        let cursors = vec![cursor_at(3, 1), cursor_at(3, 5), cursor_at(3, 10)];
        assert!(all_cursors_same_line(&cursors));
    }

    #[test]
    fn all_cursors_same_line_false() {
        let cursors = vec![cursor_at(3, 1), cursor_at(4, 5)];
        assert!(!all_cursors_same_line(&cursors));
    }

    #[test]
    fn cursor_line_span_single() {
        let cursors = vec![cursor_at(5, 1)];
        assert_eq!(cursor_line_span(&cursors), 1);
    }

    #[test]
    fn cursor_line_span_multiple() {
        let cursors = vec![cursor_at(2, 1), cursor_at(8, 1), cursor_at(5, 1)];
        assert_eq!(cursor_line_span(&cursors), 7);
    }

    #[test]
    fn cursor_line_span_empty() {
        let cursors: Vec<CursorState> = vec![];
        assert_eq!(cursor_line_span(&cursors), 0);
    }

    #[test]
    fn deduplicate_cursors_removes_dupes() {
        let cursors = vec![cursor_at(1, 1), cursor_at(2, 3), cursor_at(1, 1), cursor_at(2, 3)];
        let deduped = deduplicate_cursors(&cursors);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn deduplicate_cursors_preserves_unique() {
        let cursors = vec![cursor_at(1, 1), cursor_at(2, 2), cursor_at(3, 3)];
        let deduped = deduplicate_cursors(&cursors);
        assert_eq!(deduped.len(), 3);
    }

    #[test]
    fn cursor_bounding_box_computes() {
        let cursors = vec![cursor_at(3, 5), cursor_at(1, 2), cursor_at(7, 10)];
        let (min, max) = cursor_bounding_box(&cursors).unwrap();
        assert_eq!(min.line, 1);
        assert_eq!(min.column, 2);
        assert_eq!(max.line, 7);
        assert_eq!(max.column, 10);
    }

    #[test]
    fn cursor_bounding_box_empty() {
        let cursors: Vec<CursorState> = vec![];
        assert!(cursor_bounding_box(&cursors).is_none());
    }

    #[test]
    fn first_and_last_cursor() {
        let cursors = vec![cursor_at(5, 3), cursor_at(1, 1), cursor_at(10, 8)];
        let first = first_cursor(&cursors).unwrap();
        assert_eq!(first.position().line, 1);
        let last = last_cursor(&cursors).unwrap();
        assert_eq!(last.position().line, 10);
    }

    #[test]
    fn any_has_selection_true() {
        let cursors = vec![
            cursor_at(1, 1),
            CursorState::with_selection(Position::new(2, 1), Position::new(2, 5)),
        ];
        assert!(any_has_selection(&cursors));
        assert_eq!(selection_count(&cursors), 1);
    }

    #[test]
    fn any_has_selection_false() {
        let cursors = vec![cursor_at(1, 1), cursor_at(2, 2)];
        assert!(!any_has_selection(&cursors));
        assert_eq!(selection_count(&cursors), 0);
    }

    #[test]
    fn cursors_on_line_filters() {
        let cursors = vec![cursor_at(1, 1), cursor_at(2, 3), cursor_at(1, 5)];
        let on_1 = cursors_on_line(&cursors, 1);
        assert_eq!(on_1.len(), 2);
        let on_3 = cursors_on_line(&cursors, 3);
        assert!(on_3.is_empty());
    }

    #[test]
    fn collapse_selections_removes_selections() {
        let cursors = vec![
            CursorState::with_selection(Position::new(1, 1), Position::new(1, 5)),
            CursorState::with_selection(Position::new(2, 1), Position::new(3, 10)),
        ];
        let collapsed = collapse_selections(&cursors);
        assert!(!collapsed[0].is_selection());
        assert!(!collapsed[1].is_selection());
        assert_eq!(collapsed[0].position(), Position::new(1, 5));
    }

    #[test]
    fn cursor_state_anchor_and_collapsed() {
        let c = CursorState::with_selection(Position::new(1, 1), Position::new(1, 10));
        assert_eq!(c.anchor(), Position::new(1, 1));
        assert!(c.is_selection());
        let col = c.collapsed();
        assert!(!col.is_selection());
        assert_eq!(col.position(), Position::new(1, 10));
    }

    // -- CursorSoftWrapHandler tests ----------------------------------------

    #[test]
    fn soft_wrap_logical_to_visual_first_line() {
        let h = CursorSoftWrapHandler::new(80);
        let (vl, vc) = h.logical_to_visual(5);
        assert_eq!(vl, 0);
        assert_eq!(vc, 5);
    }

    #[test]
    fn soft_wrap_logical_to_visual_wraps() {
        let h = CursorSoftWrapHandler::new(10);
        // column 15 → visual line 1, visual col 5
        let (vl, vc) = h.logical_to_visual(15);
        assert_eq!(vl, 1);
        assert_eq!(vc, 5);
    }

    #[test]
    fn soft_wrap_roundtrip() {
        let h = CursorSoftWrapHandler::new(20);
        for col in 1..=100 {
            let (vl, vc) = h.logical_to_visual(col);
            let back = h.visual_to_logical(vl, vc);
            assert_eq!(back, col);
        }
    }

    #[test]
    fn soft_wrap_visual_line_count() {
        let h = CursorSoftWrapHandler::new(10);
        assert_eq!(h.visual_line_count(0), 1);
        assert_eq!(h.visual_line_count(10), 1);
        assert_eq!(h.visual_line_count(11), 2);
        assert_eq!(h.visual_line_count(20), 2);
        assert_eq!(h.visual_line_count(21), 3);
    }

    // -- CursorColumnMemory tests -------------------------------------------

    #[test]
    fn column_memory_default_is_none() {
        let mem = CursorColumnMemory::new();
        assert_eq!(mem.get(), None);
    }

    #[test]
    fn column_memory_set_and_resolve() {
        let mut mem = CursorColumnMemory::new();
        mem.set(20);
        assert_eq!(mem.resolve(5, 80), 20);
        assert_eq!(mem.resolve(5, 15), 15); // clamped
    }

    #[test]
    fn column_memory_clear() {
        let mut mem = CursorColumnMemory::new();
        mem.set(42);
        mem.clear();
        assert_eq!(mem.get(), None);
        assert_eq!(mem.resolve(7, 80), 7);
    }

    #[test]
    fn column_memory_apply_vertical_move() {
        let mut mem = CursorColumnMemory::new();
        let col = mem.apply_vertical_move(30, 20);
        assert_eq!(col, 20); // clamped to max
        assert_eq!(mem.get(), Some(30)); // memory preserved
        let col2 = mem.apply_vertical_move(5, 80);
        assert_eq!(col2, 30); // uses memorised value
    }

    // -- CursorWordBoundary tests -------------------------------------------

    #[test]
    fn word_boundary_default_camel_case() {
        let wb = CursorWordBoundary::new();
        assert!(wb.is_boundary(b'a', b'A')); // camelCase
        assert!(!wb.is_boundary(b'a', b'b')); // same class
        assert!(wb.is_boundary(b'a', b' ')); // word→ws
    }

    #[test]
    fn word_boundary_extra_separators() {
        let mut wb = CursorWordBoundary::new();
        wb.add_separators(b"-.");
        assert!(wb.is_boundary(b'a', b'-'));
        assert!(wb.is_boundary(b'-', b'a'));
    }

    #[test]
    fn word_boundary_find_prev() {
        let wb = CursorWordBoundary::new();
        let col = wb.find_prev_boundary("hello world", 11);
        assert_eq!(col, 7); // 'w' in "world"
    }

    #[test]
    fn word_boundary_find_next() {
        let wb = CursorWordBoundary::new();
        let col = wb.find_next_boundary("hello world", 1);
        assert_eq!(col, 6); // after "hello"
    }

    // -- CursorBlinkTimer tests ---------------------------------------------

    #[test]
    fn blink_timer_toggles() {
        let mut t = CursorBlinkTimer::new(500);
        assert!(t.is_visible());
        let changed = t.tick(500);
        assert!(changed);
        assert!(!t.is_visible());
        t.tick(500);
        assert!(t.is_visible());
    }

    #[test]
    fn blink_timer_reset() {
        let mut t = CursorBlinkTimer::new(500);
        t.tick(500);
        assert!(!t.is_visible());
        t.reset();
        assert!(t.is_visible());
    }

    #[test]
    fn blink_timer_disabled() {
        let mut t = CursorBlinkTimer::new(500);
        t.set_enabled(false);
        let changed = t.tick(1000);
        assert!(!changed);
        assert!(t.is_visible());
    }

    #[test]
    fn blink_timer_interval_change() {
        let mut t = CursorBlinkTimer::new(500);
        t.set_interval_ms(200);
        assert_eq!(t.interval_ms(), 200);
        t.tick(200);
        assert!(!t.is_visible());
    }


#[test]
    fn cursorviewportscroller_severity_ordering() {
        assert!(CursorViewportScrollerSeverity::Critical > CursorViewportScrollerSeverity::High);
        assert!(CursorViewportScrollerSeverity::High > CursorViewportScrollerSeverity::Medium);
        assert!(CursorViewportScrollerSeverity::Medium > CursorViewportScrollerSeverity::Low);
    }

    #[test]
    fn cursorviewportscroller_severity_display() {
        assert_eq!(CursorViewportScrollerSeverity::Low.to_string(), "low");
        assert_eq!(CursorViewportScrollerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn cursorviewportscroller_entry_creation() {
        let e = CursorViewportScrollerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, CursorViewportScrollerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn cursorviewportscroller_entry_builder() {
        let e = CursorViewportScrollerEntry::new("e2", "Entry 2")
            .with_severity(CursorViewportScrollerSeverity::High)
            .with_detail("some detail")
            .with_viewport_lines(42);
        assert_eq!(e.severity, CursorViewportScrollerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.viewport_lines, 42);
    }

    #[test]
    fn cursorviewportscroller_entry_enable_disable() {
        let mut e = CursorViewportScrollerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn cursorviewportscroller_add_and_count() {
        let mut mgr = CursorViewportScroller::new("test");
        mgr.add(CursorViewportScrollerEntry::new("a", "A"));
        mgr.add(CursorViewportScrollerEntry::new("b", "B").with_severity(CursorViewportScrollerSeverity::High));
        assert_eq!(mgr.viewport_lines(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn cursorviewportscroller_remove() {
        let mut mgr = CursorViewportScroller::new("test");
        mgr.add(CursorViewportScrollerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn cursorviewportscroller_capacity() {
        let mut mgr = CursorViewportScroller::new("test").with_capacity(1);
        assert!(mgr.add(CursorViewportScrollerEntry::new("a", "A")));
        assert!(!mgr.add(CursorViewportScrollerEntry::new("b", "B")));
    }

    #[test]
    fn cursorviewportscroller_sorted_by_severity() {
        let mut mgr = CursorViewportScroller::new("test");
        mgr.add(CursorViewportScrollerEntry::new("lo", "Low"));
        mgr.add(CursorViewportScrollerEntry::new("hi", "High").with_severity(CursorViewportScrollerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, CursorViewportScrollerSeverity::Critical);
    }

    #[test]
    fn cursorviewportscroller_summary() {
        let mgr = CursorViewportScroller::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn cursorselectionexpander_config_defaults() {
        let cfg = CursorSelectionExpanderConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn cursorselectionexpander_item_creation() {
        let item = CursorSelectionExpanderItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn cursorselectionexpander_add_and_get() {
        let mut mgr = CursorSelectionExpander::new(CursorSelectionExpanderConfig::new("test"));
        mgr.add(CursorSelectionExpanderItem::new("k1", "v1"));
        assert_eq!(mgr.selection_length(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn cursorselectionexpander_remove_item() {
        let mut mgr = CursorSelectionExpander::new(CursorSelectionExpanderConfig::new("test"));
        mgr.add(CursorSelectionExpanderItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn cursorselectionexpander_sorted_by_priority() {
        let mut mgr = CursorSelectionExpander::new(CursorSelectionExpanderConfig::new("test"));
        mgr.add(CursorSelectionExpanderItem::new("lo", "low").with_priority(1));
        mgr.add(CursorSelectionExpanderItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn cursorselectionexpander_items_with_tag() {
        let mut mgr = CursorSelectionExpander::new(CursorSelectionExpanderConfig::new("test"));
        mgr.add(CursorSelectionExpanderItem::new("a", "1").with_tag("x"));
        mgr.add(CursorSelectionExpanderItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn cursorselectionexpander_report() {
        let mgr = CursorSelectionExpander::new(CursorSelectionExpanderConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn cursor_entry_creation() {
        let e = CursorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn cursor_entry_with_priority() {
        let e = CursorEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn cursor_entry_metadata() {
        let e = CursorEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn cursor_entry_remove_meta() {
        let mut e = CursorEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn cursor_entry_activate_deactivate() {
        let mut e = CursorEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn cursor_config_add_sorted() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("lo", "Lo").with_priority(1));
        c.add(CursorEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn cursor_config_capacity() {
        let mut c = CursorConfig::new(1);
        assert!(c.add(CursorEntry::new("a", "A")));
        assert!(!c.add(CursorEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn cursor_config_remove() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn cursor_config_get() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn cursor_config_active_entries() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "A"));
        c.add(CursorEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn cursor_config_enable_disable() {
        let mut c = CursorConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn cursor_config_clear() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn cursor_config_find_by_label() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn cursor_config_top_n() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "A").with_priority(1));
        c.add(CursorEntry::new("b", "B").with_priority(2));
        c.add(CursorEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn cursor_config_deactivate_activate_all() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "A"));
        c.add(CursorEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn cursor_config_highest_priority() {
        let mut c = CursorConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(CursorEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn cursor_config_contains() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn cursor_config_labels() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "Alpha"));
        c.add(CursorEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn cursor_config_drain_inactive() {
        let mut c = CursorConfig::new(10);
        c.add(CursorEntry::new("a", "A"));
        c.add(CursorEntry::new("b", "B"));
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


    // xa_ extended tests for cursor
    #[test]
    fn xa_cursor_ring_new() {
        let rb = super::XaCursorRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_cursor_ring_push_len() {
        let mut rb = super::XaCursorRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_cursor_ring_wrap() {
        let mut rb = super::XaCursorRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_cursor_ring_mean_empty() {
        let rb = super::XaCursorRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_cursor_ring_mean_values() {
        let mut rb = super::XaCursorRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_cursor_ring_min_max() {
        let mut rb = super::XaCursorRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_cursor_ring_iter() {
        let mut rb = super::XaCursorRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_cursor_counter_new() {
        let c = super::XaCursorCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_cursor_counter_inc() {
        let mut c = super::XaCursorCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_cursor_counter_inc_by() {
        let mut c = super::XaCursorCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_cursor_counter_reset() {
        let mut c = super::XaCursorCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_cursor_counter_clear() {
        let mut c = super::XaCursorCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_cursor_counter_default() {
        let c = super::XaCursorCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 24 ----

    #[test]
    fn xc_24_pool_new_empty() {
        let pool: super::Xc24Pool<i32> = super::Xc24Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_24_pool_release_acquire() {
        let mut pool = super::Xc24Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_24_pool_acquire_empty() {
        let mut pool: super::Xc24Pool<i32> = super::Xc24Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_24_pool_full() {
        let mut pool = super::Xc24Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_24_pool_drain() {
        let mut pool = super::Xc24Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_24_pool_stats() {
        let mut pool = super::Xc24Pool::new(8);
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
    fn xc_24_pool_clear() {
        let mut pool = super::Xc24Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_24_pool_shrink() {
        let mut pool = super::Xc24Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_24_pool_default() {
        let pool: super::Xc24Pool<String> = super::Xc24Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_24_pool_extend() {
        let mut pool = super::Xc24Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_24_pool_retain() {
        let mut pool = super::Xc24Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_24_scheduler_round_robin() {
        let mut sched = super::Xc24Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_24_scheduler_empty() {
        let mut sched = super::Xc24Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_24_scheduler_reset() {
        let mut sched = super::Xc24Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_24_scheduler_add_remove() {
        let mut sched = super::Xc24Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_24_scheduler_targets() {
        let sched = super::Xc24Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_24_hash_empty() {
        assert_eq!(super::xc_24_hash(b""), 5381);
    }

    #[test]
    fn xc_24_hash_data() {
        let h = super::xc_24_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_24_hash(b"hello"), h);
    }

    #[test]
    fn xc_24_reverse_str() {
        assert_eq!(super::xc_24_reverse("abc"), "cba");
        assert_eq!(super::xc_24_reverse(""), "");
    }


    // --- xd_65 deepening tests ---

    #[test]
    fn xd_65_sm_initial_state() {
        let sm = Xd65StateMachine::new();
        assert_eq!(sm.current_state(), Xd65State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_65_sm_valid_idle_to_running() {
        let mut sm = Xd65StateMachine::new();
        assert!(sm.transition(Xd65State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd65State::Running);
    }

    #[test]
    fn xd_65_sm_valid_running_to_paused() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        assert!(sm.transition(Xd65State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd65State::Paused);
    }

    #[test]
    fn xd_65_sm_valid_running_to_done() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        assert!(sm.transition(Xd65State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd65State::Done);
    }

    #[test]
    fn xd_65_sm_valid_paused_to_running() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        sm.transition(Xd65State::Paused).unwrap();
        assert!(sm.transition(Xd65State::Running).is_ok());
    }

    #[test]
    fn xd_65_sm_valid_done_to_idle() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        sm.transition(Xd65State::Done).unwrap();
        assert!(sm.transition(Xd65State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd65State::Idle);
    }

    #[test]
    fn xd_65_sm_invalid_idle_to_done() {
        let mut sm = Xd65StateMachine::new();
        assert!(sm.transition(Xd65State::Done).is_err());
    }

    #[test]
    fn xd_65_sm_invalid_idle_to_paused() {
        let mut sm = Xd65StateMachine::new();
        assert!(sm.transition(Xd65State::Paused).is_err());
    }

    #[test]
    fn xd_65_sm_history_tracking() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        sm.transition(Xd65State::Paused).unwrap();
        sm.transition(Xd65State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd65State::Idle);
        assert_eq!(sm.history()[0].to, Xd65State::Running);
        assert_eq!(sm.history()[1].from, Xd65State::Running);
        assert_eq!(sm.history()[2].to, Xd65State::Done);
    }

    #[test]
    fn xd_65_sm_serialize_deserialize() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd65StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd65State::Running));
    }

    #[test]
    fn xd_65_sm_deserialize_invalid() {
        assert_eq!(Xd65StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_65_sm_reset() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd65State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_65_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd65EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd65Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_65_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd65EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd65Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd65Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_65_bus_unsubscribe() {
        let mut bus = Xd65EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_65_event_kind_and_payload() {
        let e = Xd65Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd65Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_65_bus_clear_history() {
        let mut bus = Xd65EventBus::new();
        bus.publish(Xd65Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_65_sm_step_counter_increments() {
        let mut sm = Xd65StateMachine::new();
        sm.transition(Xd65State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd65State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #66 --

    #[test]
    fn xf66_trie_insert_search() {
        let mut t = Xf66Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf66_trie_starts_with() {
        let mut t = Xf66Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf66_trie_remove() {
        let mut t = Xf66Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf66_trie_word_count() {
        let mut t = Xf66Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf66_trie_longest_prefix() {
        let mut t = Xf66Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf66_trie_all_words() {
        let mut t = Xf66Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf66_trie_autocomplete() {
        let mut t = Xf66Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf66_trie_empty_search() {
        let t = Xf66Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf66_bloom_add_contains() {
        let mut bf = Xf66BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf66_bloom_probably_absent() {
        let bf = Xf66BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf66_bloom_false_positive_rate() {
        let mut bf = Xf66BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf66_bloom_clear() {
        let mut bf = Xf66BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf66_bloom_union() {
        let mut a = Xf66BloomFilter::xf_new(512, 2);
        let mut b = Xf66BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf66_bloom_intersection_estimate() {
        let mut a = Xf66BloomFilter::xf_new(512, 2);
        let mut b = Xf66BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf66_bloom_union_size_mismatch() {
        let a = Xf66BloomFilter::xf_new(256, 2);
        let b = Xf66BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh23_skip_insert_contains() {
        let mut sl = super::Xh23SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh23_skip_remove() {
        let mut sl = super::Xh23SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh23_skip_len() {
        let mut sl = super::Xh23SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh23_skip_range_query() {
        let mut sl = super::Xh23SkipList::xh_new(4);
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
    fn xh23_skip_floor_ceiling() {
        let mut sl = super::Xh23SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh23_skip_rank() {
        let mut sl = super::Xh23SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh23_skip_empty() {
        let sl = super::Xh23SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh23_skip_duplicates() {
        let mut sl = super::Xh23SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh23_bitset_set_test() {
        let mut bs = super::Xh23BitSet::xh_new(256);
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
    fn xh23_bitset_clear_count() {
        let mut bs = super::Xh23BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh23_bitset_and_or_xor() {
        let mut a = super::Xh23BitSet::xh_new(128);
        let mut b = super::Xh23BitSet::xh_new(128);
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
    fn xh23_bitset_iter_ones() {
        let mut bs = super::Xh23BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh23_bitset_first_last() {
        let mut bs = super::Xh23BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh23_bitset_empty() {
        let bs = super::Xh23BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi23_deque_push_pop_back() {
        let mut dq = super::Xi23Deque::xi_new(4);
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
    fn xi23_deque_push_pop_front() {
        let mut dq = super::Xi23Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi23_deque_mixed_ops() {
        let mut dq = super::Xi23Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi23_deque_get_and_split() {
        let mut dq = super::Xi23Deque::xi_new(8);
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
    fn xi23_deque_rotate_left() {
        let mut dq = super::Xi23Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi23_deque_rotate_right() {
        let mut dq = super::Xi23Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi23_deque_grow() {
        let mut dq = super::Xi23Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi23_deque_empty() {
        let dq = super::Xi23Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi23_interval_tree_insert_query() {
        let mut tree = super::Xi23IntervalTree::xi_new();
        tree.xi_insert(super::Xi23Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi23Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi23Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi23_interval_tree_overlap() {
        let mut tree = super::Xi23IntervalTree::xi_new();
        tree.xi_insert(super::Xi23Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi23Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi23Interval::xi_new(12, 20));
        let q = super::Xi23Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi23_interval_tree_remove() {
        let mut tree = super::Xi23IntervalTree::xi_new();
        tree.xi_insert(super::Xi23Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi23Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi23_interval_tree_gaps() {
        let mut tree = super::Xi23IntervalTree::xi_new();
        tree.xi_insert(super::Xi23Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi23Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi23Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi23Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi23Interval::xi_new(8, 10));
    }

    #[test]
    fn xi23_interval_tree_merge() {
        let mut tree = super::Xi23IntervalTree::xi_new();
        tree.xi_insert(super::Xi23Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi23Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi23Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi23Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi23Interval::xi_new(10, 15));
    }

    #[test]
    fn xi23_interval_tree_all() {
        let mut tree = super::Xi23IntervalTree::xi_new();
        tree.xi_insert(super::Xi23Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi23Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi23_interval_tree_empty() {
        let tree = super::Xi23IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi23_interval_tree_contains_point() {
        let iv = super::Xi23Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 23) ---

    #[test]
    fn xj_23_uf_make_and_find() {
        let mut uf = super::Xj23UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_23_uf_union_connected() {
        let mut uf = super::Xj23UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_23_uf_component_count() {
        let mut uf = super::Xj23UnionFind::xj_new();
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
    fn xj_23_uf_component_size() {
        let mut uf = super::Xj23UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_23_uf_largest_component() {
        let mut uf = super::Xj23UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_23_uf_many_elements() {
        let mut uf = super::Xj23UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_23_uf_separate_components() {
        let mut uf = super::Xj23UnionFind::xj_new();
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
    fn xj_23_uf_path_compression() {
        let mut uf = super::Xj23UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_23_bt_insert_get() {
        let mut bt = super::Xj23BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_23_bt_contains_len() {
        let mut bt = super::Xj23BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_23_bt_replace() {
        let mut bt = super::Xj23BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_23_bt_remove() {
        let mut bt = super::Xj23BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_23_bt_keys_values() {
        let mut bt = super::Xj23BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_23_bt_range() {
        let mut bt = super::Xj23BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_23_bt_min_max() {
        let mut bt = super::Xj23BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_23_bt_many_inserts() {
        let mut bt = super::Xj23BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
