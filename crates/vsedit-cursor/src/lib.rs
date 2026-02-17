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
}
