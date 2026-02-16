//! Editor input controller — translates input events into editor operations.
//!
//! [`EditorController`] is the bridge between key-press events and the
//! cursor / text-model commands.  It owns a [`TextModel`] and a
//! [`CursorController`] directly (both are `!Sync` due to interior
//! mutability, so `RwLock` is inappropriate).

use vsedit_cursor::{
    self, CursorController, CursorState,
};
use vsedit_editor_types::{ITextModel, Position, Range, Selection};
use vsedit_text_model::TextModel;

// ---------------------------------------------------------------------------
// EditorAction
// ---------------------------------------------------------------------------

/// Every high-level editing action the controller can execute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorAction {
    // -- cursor movement (collapse selection) --
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorUp,
    MoveCursorDown,
    MoveCursorWordLeft,
    MoveCursorWordRight,
    MoveCursorLineStart,
    MoveCursorLineEnd,
    MoveCursorDocumentStart,
    MoveCursorDocumentEnd,

    // -- selection (extend selection) --
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    SelectAll,

    // -- text mutation --
    InsertText(String),
    DeleteLeft,
    DeleteRight,
    DeleteWordLeft,
    DeleteWordRight,
    DeleteLine,
    NewLine,

    // -- indentation --
    IndentLine,
    OutdentLine,

    // -- history --
    Undo,
    Redo,
}

// ---------------------------------------------------------------------------
// EditorController
// ---------------------------------------------------------------------------

/// Translates [`EditorAction`]s into cursor and text-model mutations.
pub struct EditorController {
    pub model: TextModel,
    pub cursors: CursorController,
}

impl EditorController {
    /// Create a controller with the given initial text.
    pub fn new(text: &str) -> Self {
        Self {
            model: TextModel::new(text),
            cursors: CursorController::new(),
        }
    }

    /// Dispatch a single action.
    pub fn execute_action(&mut self, action: EditorAction) {
        match action {
            // -- movement -------------------------------------------------------
            EditorAction::MoveCursorLeft => self.move_cursors(|m, c| vsedit_cursor::move_left(m, c, false, 1)),
            EditorAction::MoveCursorRight => self.move_cursors(|m, c| vsedit_cursor::move_right(m, c, false, 1)),
            EditorAction::MoveCursorUp => self.move_cursors_vertical(false, true),
            EditorAction::MoveCursorDown => self.move_cursors_vertical(false, false),
            EditorAction::MoveCursorWordLeft => self.move_cursors(|m, c| vsedit_cursor::move_word_left(m, c, false)),
            EditorAction::MoveCursorWordRight => self.move_cursors(|m, c| vsedit_cursor::move_word_right(m, c, false)),
            EditorAction::MoveCursorLineStart => self.move_cursors(|m, c| vsedit_cursor::move_to_line_start(m, c, false)),
            EditorAction::MoveCursorLineEnd => self.move_cursors(|m, c| vsedit_cursor::move_to_line_end(m, c, false)),
            EditorAction::MoveCursorDocumentStart => self.move_cursors(|m, c| vsedit_cursor::move_to_document_start(m, c, false)),
            EditorAction::MoveCursorDocumentEnd => self.move_cursors(|m, c| vsedit_cursor::move_to_document_end(m, c, false)),

            // -- selection ------------------------------------------------------
            EditorAction::SelectLeft => self.move_cursors(|m, c| vsedit_cursor::move_left(m, c, true, 1)),
            EditorAction::SelectRight => self.move_cursors(|m, c| vsedit_cursor::move_right(m, c, true, 1)),
            EditorAction::SelectUp => self.move_cursors_vertical(true, true),
            EditorAction::SelectDown => self.move_cursors_vertical(true, false),
            EditorAction::SelectAll => self.select_all(),

            // -- text mutation --------------------------------------------------
            EditorAction::InsertText(ref text) => self.insert_text(text),
            EditorAction::DeleteLeft => self.delete_left(),
            EditorAction::DeleteRight => self.delete_right(),
            EditorAction::DeleteWordLeft => self.delete_word_left(),
            EditorAction::DeleteWordRight => self.delete_word_right(),
            EditorAction::DeleteLine => self.delete_line(),
            EditorAction::NewLine => self.insert_text("\n"),

            // -- indentation ----------------------------------------------------
            EditorAction::IndentLine => self.indent_line(),
            EditorAction::OutdentLine => self.outdent_line(),

            // -- history --------------------------------------------------------
            EditorAction::Undo => { self.model.undo(); }
            EditorAction::Redo => { self.model.redo(); }
        }
    }

    // -- helpers: cursor movement -------------------------------------------

    /// Apply a simple (non-vertical) movement function to every cursor.
    fn move_cursors<F>(&mut self, f: F)
    where
        F: Fn(&dyn ITextModel, &CursorState) -> CursorState,
    {
        let count = self.cursors.get_all().len();
        for i in 0..count {
            let cur = self.cursors.get_all()[i].clone();
            let new_state = f(&self.model, &cur);
            self.cursors.set_state(i, new_state);
            self.cursors.set_column_memory(i, None);
        }
        self.cursors.merge_overlapping();
    }

    /// Apply vertical movement (up/down) preserving column memory.
    fn move_cursors_vertical(&mut self, select: bool, up: bool) {
        let count = self.cursors.get_all().len();
        for i in 0..count {
            let cur = self.cursors.get_all()[i].clone();
            let mem = self.cursors.get_column_memory(i);
            let (new_state, new_mem) = if up {
                vsedit_cursor::move_up(&self.model, &cur, select, 1, mem)
            } else {
                vsedit_cursor::move_down(&self.model, &cur, select, 1, mem)
            };
            self.cursors.set_state(i, new_state);
            self.cursors.set_column_memory(i, Some(new_mem));
        }
        self.cursors.merge_overlapping();
    }

    fn select_all(&mut self) {
        let last_line = self.model.get_line_count();
        let last_col = self.model.get_line_max_column(last_line);
        let start = Position::new(1, 1);
        let end = Position::new(last_line, last_col);
        self.cursors.set_state(
            0,
            CursorState {
                selection: Selection::from_positions(start, end),
            },
        );
    }

    // -- helpers: text mutation ----------------------------------------------

    /// Insert `text` at every cursor, replacing any selection, then shift
    /// cursors so they sit after the inserted text.
    fn insert_text(&mut self, text: &str) {
        // Process cursors from bottom to top so earlier positions stay valid.
        let mut positions: Vec<(usize, Range)> = self
            .cursors
            .get_all()
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.selection.as_range()))
            .collect();
        positions.sort_by(|a, b| b.1.start.cmp(&a.1.start));

        let mut new_positions: Vec<(usize, Position)> = Vec::new();
        for (i, range) in &positions {
            self.model.apply_edit(*range, text);
            // Compute the position after the inserted text.
            let after = compute_end_position(range.start, text);
            new_positions.push((*i, after));
        }

        for (i, pos) in new_positions {
            self.cursors.set_state(i, CursorState::from_position(pos));
            self.cursors.set_column_memory(i, None);
        }
        self.cursors.merge_overlapping();
    }

    /// Backspace — delete one character (or selection) left of each cursor.
    fn delete_left(&mut self) {
        let mut ranges: Vec<(usize, Range)> = self
            .cursors
            .get_all()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let sel = c.selection.as_range();
                if !sel.is_empty() {
                    (i, sel)
                } else {
                    let before = vsedit_cursor::move_left(&self.model, c, false, 1);
                    (
                        i,
                        Range::from_positions(before.position(), c.position()),
                    )
                }
            })
            .collect();
        ranges.sort_by(|a, b| b.1.start.cmp(&a.1.start));

        let mut new_positions: Vec<(usize, Position)> = Vec::new();
        for (i, range) in &ranges {
            self.model.delete(*range);
            new_positions.push((*i, range.start));
        }

        for (i, pos) in new_positions {
            self.cursors.set_state(i, CursorState::from_position(pos));
        }
        self.cursors.merge_overlapping();
    }

    /// Delete — delete one character (or selection) right of each cursor.
    fn delete_right(&mut self) {
        let mut ranges: Vec<(usize, Range)> = self
            .cursors
            .get_all()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let sel = c.selection.as_range();
                if !sel.is_empty() {
                    (i, sel)
                } else {
                    let after = vsedit_cursor::move_right(&self.model, c, false, 1);
                    (
                        i,
                        Range::from_positions(c.position(), after.position()),
                    )
                }
            })
            .collect();
        ranges.sort_by(|a, b| b.1.start.cmp(&a.1.start));

        let mut new_positions: Vec<(usize, Position)> = Vec::new();
        for (i, range) in &ranges {
            self.model.delete(*range);
            new_positions.push((*i, range.start));
        }

        for (i, pos) in new_positions {
            self.cursors.set_state(i, CursorState::from_position(pos));
        }
        self.cursors.merge_overlapping();
    }

    /// Delete the word to the left of each cursor.
    fn delete_word_left(&mut self) {
        let mut ranges: Vec<(usize, Range)> = self
            .cursors
            .get_all()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let sel = c.selection.as_range();
                if !sel.is_empty() {
                    (i, sel)
                } else {
                    let word_start = vsedit_cursor::move_word_left(&self.model, c, false);
                    (
                        i,
                        Range::from_positions(word_start.position(), c.position()),
                    )
                }
            })
            .collect();
        ranges.sort_by(|a, b| b.1.start.cmp(&a.1.start));

        let mut new_positions: Vec<(usize, Position)> = Vec::new();
        for (i, range) in &ranges {
            self.model.delete(*range);
            new_positions.push((*i, range.start));
        }

        for (i, pos) in new_positions {
            self.cursors.set_state(i, CursorState::from_position(pos));
        }
        self.cursors.merge_overlapping();
    }

    /// Delete the word to the right of each cursor.
    fn delete_word_right(&mut self) {
        let mut ranges: Vec<(usize, Range)> = self
            .cursors
            .get_all()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let sel = c.selection.as_range();
                if !sel.is_empty() {
                    (i, sel)
                } else {
                    let word_end = vsedit_cursor::move_word_right(&self.model, c, false);
                    (
                        i,
                        Range::from_positions(c.position(), word_end.position()),
                    )
                }
            })
            .collect();
        ranges.sort_by(|a, b| b.1.start.cmp(&a.1.start));

        let mut new_positions: Vec<(usize, Position)> = Vec::new();
        for (i, range) in &ranges {
            self.model.delete(*range);
            new_positions.push((*i, range.start));
        }

        for (i, pos) in new_positions {
            self.cursors.set_state(i, CursorState::from_position(pos));
        }
        self.cursors.merge_overlapping();
    }

    /// Delete the entire line at each cursor position.
    fn delete_line(&mut self) {
        let mut lines: Vec<u32> = self
            .cursors
            .get_all()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        // Delete from bottom to top.
        for &line in lines.iter().rev() {
            let line_count = self.model.get_line_count();
            if line_count == 1 {
                // Only line — clear it.
                let max_col = self.model.get_line_max_column(1);
                self.model.delete(Range::new(1, 1, 1, max_col));
            } else if line == line_count {
                // Last line — include the preceding newline.
                let prev_max = self.model.get_line_max_column(line - 1);
                let cur_max = self.model.get_line_max_column(line);
                self.model.delete(Range::new(line - 1, prev_max, line, cur_max));
            } else {
                // Delete line including its trailing newline.
                let max_col = self.model.get_line_max_column(line);
                self.model.delete(Range::new(line, 1, line + 1, 1));
                let _ = max_col; // suppress unused
            }
        }

        // Place all cursors at line start after deletion.
        let line_count = self.model.get_line_count();
        for i in 0..self.cursors.get_all().len() {
            let cur_line = self.cursors.get_all()[i].position().line.min(line_count);
            self.cursors.set_state(i, CursorState::from_position(Position::new(cur_line, 1)));
        }
        self.cursors.merge_overlapping();
    }

    /// Add one level of indentation (a tab character) at the start of each
    /// cursor's line.
    fn indent_line(&mut self) {
        let mut lines: Vec<u32> = self
            .cursors
            .get_all()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for &line in &lines {
            self.model.insert(Position::new(line, 1), "\t");
        }

        // Shift cursors right by 1 column on affected lines.
        for i in 0..self.cursors.get_all().len() {
            let pos = self.cursors.get_all()[i].position();
            if lines.contains(&pos.line) {
                self.cursors.set_state(
                    i,
                    CursorState::from_position(Position::new(pos.line, pos.column + 1)),
                );
            }
        }
    }

    /// Remove one level of indentation (leading tab or up to 4 spaces) from
    /// each cursor's line.
    fn outdent_line(&mut self) {
        let mut lines: Vec<u32> = self
            .cursors
            .get_all()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        let mut removed: Vec<(u32, u32)> = Vec::new();
        for &line in &lines {
            let content = self.model.get_line_content(line).to_string();
            let remove_cols = if content.starts_with('\t') {
                1
            } else {
                // Remove up to 4 leading spaces.
                let spaces: u32 = content
                    .bytes()
                    .take(4)
                    .take_while(|&b| b == b' ')
                    .count() as u32;
                spaces
            };
            if remove_cols > 0 {
                self.model.delete(Range::new(line, 1, line, 1 + remove_cols));
                removed.push((line, remove_cols));
            }
        }

        // Shift cursors left on affected lines.
        for i in 0..self.cursors.get_all().len() {
            let pos = self.cursors.get_all()[i].position();
            for &(line, cols) in &removed {
                if pos.line == line {
                    let new_col = if pos.column > cols { pos.column - cols } else { 1 };
                    self.cursors.set_state(
                        i,
                        CursorState::from_position(Position::new(pos.line, new_col)),
                    );
                }
            }
        }
    }
}

impl Default for EditorController {
    fn default() -> Self {
        Self::new("")
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the position after inserting `text` starting at `start`.
fn compute_end_position(start: Position, text: &str) -> Position {
    let mut line = start.line;
    let mut col = start.column;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += ch.len_utf8() as u32;
        }
    }
    Position::new(line, col)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl(text: &str) -> EditorController {
        EditorController::new(text)
    }

    // 1
    #[test]
    fn default_state() {
        let c = EditorController::default();
        assert_eq!(c.model.get_value(), "");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 1));
    }

    // 2
    #[test]
    fn insert_text_basic() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("hello".into()));
        assert_eq!(c.model.get_value(), "hello");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 6));
    }

    // 3
    #[test]
    fn move_cursor_right_and_left() {
        let mut c = ctrl("abc");
        c.execute_action(EditorAction::MoveCursorRight);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
        c.execute_action(EditorAction::MoveCursorRight);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 3));
        c.execute_action(EditorAction::MoveCursorLeft);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    // 4
    #[test]
    fn move_cursor_up_and_down() {
        let mut c = ctrl("aaa\nbbb\nccc");
        // Place cursor at (1, 2)
        c.execute_action(EditorAction::MoveCursorRight);
        c.execute_action(EditorAction::MoveCursorDown);
        assert_eq!(c.cursors.get_primary().position(), Position::new(2, 2));
        c.execute_action(EditorAction::MoveCursorDown);
        assert_eq!(c.cursors.get_primary().position(), Position::new(3, 2));
        c.execute_action(EditorAction::MoveCursorUp);
        assert_eq!(c.cursors.get_primary().position(), Position::new(2, 2));
    }

    // 5
    #[test]
    fn delete_left_backspace() {
        let mut c = ctrl("hello");
        // Move to end
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 6)));
        c.execute_action(EditorAction::DeleteLeft);
        assert_eq!(c.model.get_value(), "hell");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 5));
    }

    // 6
    #[test]
    fn delete_right() {
        let mut c = ctrl("hello");
        c.execute_action(EditorAction::DeleteRight);
        assert_eq!(c.model.get_value(), "ello");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 1));
    }

    // 7
    #[test]
    fn new_line() {
        let mut c = ctrl("helloworld");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 6)));
        c.execute_action(EditorAction::NewLine);
        assert_eq!(c.model.get_value(), "hello\nworld");
        assert_eq!(c.cursors.get_primary().position(), Position::new(2, 1));
    }

    // 8
    #[test]
    fn undo_redo() {
        let mut c = ctrl("hello");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 6)));
        c.execute_action(EditorAction::InsertText(" world".into()));
        assert_eq!(c.model.get_value(), "hello world");
        c.execute_action(EditorAction::Undo);
        assert_eq!(c.model.get_value(), "hello");
        c.execute_action(EditorAction::Redo);
        assert_eq!(c.model.get_value(), "hello world");
    }

    // 9
    #[test]
    fn select_all() {
        let mut c = ctrl("hello\nworld");
        c.execute_action(EditorAction::SelectAll);
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.anchor, Position::new(1, 1));
        assert_eq!(sel.active, Position::new(2, 6));
    }

    // 10
    #[test]
    fn select_left_extends_selection() {
        let mut c = ctrl("hello");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 4)));
        c.execute_action(EditorAction::SelectLeft);
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.anchor, Position::new(1, 4));
        assert_eq!(sel.active, Position::new(1, 3));
    }

    // 11
    #[test]
    fn select_right_extends_selection() {
        let mut c = ctrl("hello");
        c.execute_action(EditorAction::SelectRight);
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.anchor, Position::new(1, 1));
        assert_eq!(sel.active, Position::new(1, 2));
    }

    // 12
    #[test]
    fn move_cursor_word_left_right() {
        let mut c = ctrl("hello world");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 1)));
        c.execute_action(EditorAction::MoveCursorWordRight);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 6));
        c.execute_action(EditorAction::MoveCursorWordLeft);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 1));
    }

    // 13
    #[test]
    fn move_cursor_line_start_end() {
        let mut c = ctrl("  hello");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 8)));
        c.execute_action(EditorAction::MoveCursorLineStart);
        // Should go to first non-whitespace (column 3)
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 3));
        c.execute_action(EditorAction::MoveCursorLineEnd);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 8));
    }

    // 14
    #[test]
    fn move_cursor_document_start_end() {
        let mut c = ctrl("aaa\nbbb\nccc");
        c.cursors.set_state(0, CursorState::from_position(Position::new(2, 2)));
        c.execute_action(EditorAction::MoveCursorDocumentEnd);
        assert_eq!(c.cursors.get_primary().position(), Position::new(3, 4));
        c.execute_action(EditorAction::MoveCursorDocumentStart);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 1));
    }

    // 15
    #[test]
    fn delete_word_left() {
        let mut c = ctrl("hello world");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 12)));
        c.execute_action(EditorAction::DeleteWordLeft);
        assert_eq!(c.model.get_value(), "hello ");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 7));
    }

    // 16
    #[test]
    fn delete_word_right() {
        let mut c = ctrl("hello world");
        c.execute_action(EditorAction::DeleteWordRight);
        assert_eq!(c.model.get_value(), " world");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 1));
    }

    // 17
    #[test]
    fn delete_line() {
        let mut c = ctrl("aaa\nbbb\nccc");
        c.cursors.set_state(0, CursorState::from_position(Position::new(2, 1)));
        c.execute_action(EditorAction::DeleteLine);
        assert_eq!(c.model.get_value(), "aaa\nccc");
    }

    // 18
    #[test]
    fn indent_line() {
        let mut c = ctrl("hello");
        c.execute_action(EditorAction::IndentLine);
        assert_eq!(c.model.get_value(), "\thello");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    // 19
    #[test]
    fn outdent_line_tab() {
        let mut c = ctrl("\thello");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 3)));
        c.execute_action(EditorAction::OutdentLine);
        assert_eq!(c.model.get_value(), "hello");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    // 20
    #[test]
    fn outdent_line_spaces() {
        let mut c = ctrl("    hello");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 6)));
        c.execute_action(EditorAction::OutdentLine);
        assert_eq!(c.model.get_value(), "hello");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    // 21
    #[test]
    fn insert_replaces_selection() {
        let mut c = ctrl("hello world");
        c.cursors.set_state(
            0,
            CursorState {
                selection: Selection::from_positions(
                    Position::new(1, 7),
                    Position::new(1, 12),
                ),
            },
        );
        c.execute_action(EditorAction::InsertText("rust".into()));
        assert_eq!(c.model.get_value(), "hello rust");
    }

    // 22
    #[test]
    fn multi_cursor_insert() {
        let mut c = ctrl("aaa\nbbb");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 1)));
        c.cursors.add_cursor(Position::new(2, 1));
        c.execute_action(EditorAction::InsertText("X".into()));
        assert_eq!(c.model.get_value(), "Xaaa\nXbbb");
    }

    // 23
    #[test]
    fn delete_left_with_selection_deletes_selection() {
        let mut c = ctrl("hello world");
        c.cursors.set_state(
            0,
            CursorState {
                selection: Selection::from_positions(
                    Position::new(1, 1),
                    Position::new(1, 6),
                ),
            },
        );
        c.execute_action(EditorAction::DeleteLeft);
        assert_eq!(c.model.get_value(), " world");
    }

    // 24
    #[test]
    fn select_up_and_down() {
        let mut c = ctrl("aaa\nbbb\nccc");
        c.cursors.set_state(0, CursorState::from_position(Position::new(2, 2)));
        c.execute_action(EditorAction::SelectDown);
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.anchor, Position::new(2, 2));
        assert_eq!(sel.active, Position::new(3, 2));
        c.execute_action(EditorAction::SelectUp);
        // Should go back to line 2
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.active, Position::new(2, 2));
    }

    // 25
    #[test]
    fn delete_line_single_line_document() {
        let mut c = ctrl("hello");
        c.execute_action(EditorAction::DeleteLine);
        assert_eq!(c.model.get_value(), "");
    }

    // 26
    #[test]
    fn delete_line_last_line() {
        let mut c = ctrl("aaa\nbbb");
        c.cursors.set_state(0, CursorState::from_position(Position::new(2, 1)));
        c.execute_action(EditorAction::DeleteLine);
        assert_eq!(c.model.get_value(), "aaa");
    }

    // 27
    #[test]
    fn compute_end_position_multiline() {
        let pos = compute_end_position(Position::new(1, 1), "ab\ncd\ne");
        assert_eq!(pos, Position::new(3, 2));
    }
}
