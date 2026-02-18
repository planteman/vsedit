//! Editor input controller — translates input events into editor operations.
//!
//! [`EditorController`] is the bridge between key-press events and the
//! cursor / text-model commands.  It owns a [`TextModel`] and a
//! [`CursorController`] directly (both are `!Sync` due to interior
//! mutability, so `RwLock` is inappropriate).

use std::collections::HashMap;
use std::fmt;
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

    // -- line operations --
    MoveLineUp,
    MoveLineDown,
    ToggleLineComment,
    SelectLine,
    InsertLineBelow,
    InsertLineAbove,

    // -- multi-cursor --
    AddCursorAbove,
    AddCursorBelow,
    AddSelectionToNextFindMatch,
    SelectAllOccurrences,
    CursorUndo,
    RemoveSecondaryCursors,

    // -- navigation --
    PageUp(u32),
    PageDown(u32),
    JumpToMatchingBracket,
    GoToLine(u32),

    // -- clipboard --
    Copy,
    Cut,
    Paste(String),

    // -- find/replace --
    Find(String),
    FindNext,
    FindPrevious,
    Replace(String, String),
    ReplaceAll(String, String),

    // -- history --
    Undo,
    Redo,

    // -- auto-close --
    ToggleAutoClose,
}

// ---------------------------------------------------------------------------
// EditorController
// ---------------------------------------------------------------------------

/// Translates [`EditorAction`]s into cursor and text-model mutations.
pub struct EditorController {
    pub model: TextModel,
    pub cursors: CursorController,
    pub clipboard: String,
    pub find_results: Vec<(usize, usize)>,
    pub find_index: usize,
    pub auto_close_pairs: Vec<(char, char)>,
    pub auto_close_enabled: bool,
}

impl EditorController {
    /// Create a controller with the given initial text.
    pub fn new(text: &str) -> Self {
        Self {
            model: TextModel::new(text),
            cursors: CursorController::new(),
            clipboard: String::new(),
            find_results: Vec::new(),
            find_index: 0,
            auto_close_pairs: vec![
                ('(', ')'),
                ('[', ']'),
                ('{', '}'),
                ('"', '"'),
                ('\'', '\''),
                ('`', '`'),
            ],
            auto_close_enabled: true,
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
            EditorAction::InsertText(ref text) => {
                if self.auto_close_enabled && text.chars().count() == 1 {
                    let ch = text.chars().next().unwrap();
                    if let Some(&(_, close)) = self.auto_close_pairs.iter().find(|(open, _)| *open == ch) {
                        let pair = format!("{}{}", ch, close);
                        self.insert_text(&pair);
                        self.move_cursors(|m, c| vsedit_cursor::move_left(m, c, false, 1));
                        return;
                    }
                }
                self.insert_text(text);
            }
            EditorAction::DeleteLeft => self.delete_left(),
            EditorAction::DeleteRight => self.delete_right(),
            EditorAction::DeleteWordLeft => self.delete_word_left(),
            EditorAction::DeleteWordRight => self.delete_word_right(),
            EditorAction::DeleteLine => self.delete_line(),
            EditorAction::NewLine => self.insert_text("\n"),

            // -- indentation ----------------------------------------------------
            EditorAction::IndentLine => self.indent_line(),
            EditorAction::OutdentLine => self.outdent_line(),

            // -- line operations ------------------------------------------------
            EditorAction::MoveLineUp => self.move_line_up(),
            EditorAction::MoveLineDown => self.move_line_down(),
            EditorAction::ToggleLineComment => self.toggle_line_comment(),
            EditorAction::SelectLine => self.select_line(),
            EditorAction::InsertLineBelow => self.insert_line_below(),
            EditorAction::InsertLineAbove => self.insert_line_above(),

            // -- multi-cursor ---------------------------------------------------
            EditorAction::AddCursorAbove => {
                self.cursors.add_cursor_above(&self.model);
            }
            EditorAction::AddCursorBelow => {
                self.cursors.add_cursor_below(&self.model);
            }
            EditorAction::AddSelectionToNextFindMatch => self.add_selection_to_next_find_match(),
            EditorAction::SelectAllOccurrences => self.select_all_occurrences(),
            EditorAction::CursorUndo => {
                self.cursors.cursor_undo();
            }
            EditorAction::RemoveSecondaryCursors => {
                self.cursors.remove_secondary_cursors();
            }

            // -- navigation -----------------------------------------------------
            EditorAction::PageUp(lines) => self.page_up(lines),
            EditorAction::PageDown(lines) => self.page_down(lines),
            EditorAction::JumpToMatchingBracket => self.jump_to_matching_bracket(),
            EditorAction::GoToLine(line) => self.go_to_line(line),

            // -- clipboard --------------------------------------------------
            EditorAction::Copy => self.copy(),
            EditorAction::Cut => self.cut(),
            EditorAction::Paste(ref text) => self.insert_text(text),

            // -- find/replace -----------------------------------------------
            EditorAction::Find(ref query) => self.find(query),
            EditorAction::FindNext => self.find_next(),
            EditorAction::FindPrevious => self.find_previous(),
            EditorAction::Replace(ref search, ref replacement) => {
                let s = search.clone();
                let r = replacement.clone();
                self.replace_current(&s, &r);
            }
            EditorAction::ReplaceAll(ref search, ref replacement) => {
                let s = search.clone();
                let r = replacement.clone();
                self.replace_all(&s, &r);
            }

            // -- history --------------------------------------------------------
            EditorAction::Undo => { self.model.undo(); }
            EditorAction::Redo => { self.model.redo(); }

            // -- auto-close -----------------------------------------------------
            EditorAction::ToggleAutoClose => {
                self.auto_close_enabled = !self.auto_close_enabled;
            }
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
    /// When auto-close is enabled and cursor sits between a matching pair,
    /// both characters are removed.
    fn delete_left(&mut self) {
        let auto_close = self.auto_close_enabled;
        let pairs = self.auto_close_pairs.clone();
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
                    let pos = c.position();
                    // Check for auto-close pair deletion
                    if auto_close && pos.column > 1 {
                        let line_content = self.model.get_line_content(pos.line);
                        let col_idx = (pos.column as usize).saturating_sub(1);
                        let bytes = line_content.as_bytes();
                        if col_idx > 0 && col_idx < bytes.len() {
                            let before = bytes[col_idx - 1] as char;
                            let after = bytes[col_idx] as char;
                            if pairs.iter().any(|&(open, close)| open == before && close == after) {
                                return (
                                    i,
                                    Range::new(pos.line, pos.column - 1, pos.line, pos.column + 1),
                                );
                            }
                        }
                    }
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

    // -- line operations ----------------------------------------------------

    /// Swap the current line with the line above.
    fn move_line_up(&mut self) {
        let line = self.cursors.get_primary().position().line;
        if line <= 1 {
            return;
        }
        let cur_content = self.model.get_line_content(line).to_string();
        let above_content = self.model.get_line_content(line - 1).to_string();
        let cur_max = self.model.get_line_max_column(line);
        let above_max = self.model.get_line_max_column(line - 1);
        // Replace the above line with current, and current with above.
        self.model.apply_edit(
            Range::new(line - 1, 1, line, cur_max),
            &format!("{}\n{}", cur_content, above_content),
        );
        // Move cursor up.
        let pos = self.cursors.get_primary().position();
        let _ = (cur_max, above_max);
        self.cursors.set_state(
            0,
            CursorState::from_position(Position::new(line - 1, pos.column)),
        );
    }

    /// Swap the current line with the line below.
    fn move_line_down(&mut self) {
        let line = self.cursors.get_primary().position().line;
        let line_count = self.model.get_line_count();
        if line >= line_count {
            return;
        }
        let cur_content = self.model.get_line_content(line).to_string();
        let below_content = self.model.get_line_content(line + 1).to_string();
        let below_max = self.model.get_line_max_column(line + 1);
        // Replace current and below lines.
        self.model.apply_edit(
            Range::new(line, 1, line + 1, below_max),
            &format!("{}\n{}", below_content, cur_content),
        );
        // Move cursor down.
        let pos = self.cursors.get_primary().position();
        self.cursors.set_state(
            0,
            CursorState::from_position(Position::new(line + 1, pos.column)),
        );
    }

    /// Toggle "// " line comment on each cursor's line.
    fn toggle_line_comment(&mut self) {
        let mut lines: Vec<u32> = self
            .cursors
            .get_all()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        // Determine if we should comment or uncomment: if all lines start
        // with "// ", uncomment; otherwise comment.
        let all_commented = lines.iter().all(|&l| {
            let content = self.model.get_line_content(l);
            let trimmed = content.trim_start();
            trimmed.starts_with("// ")
        });

        if all_commented {
            // Uncomment: remove first occurrence of "// " (preserving leading whitespace).
            for &line in lines.iter().rev() {
                let content = self.model.get_line_content(line).to_string();
                let ws_len = content.len() - content.trim_start().len();
                let start_col = (ws_len as u32) + 1;
                self.model.delete(Range::new(line, start_col, line, start_col + 3));
            }
        } else {
            // Comment: prepend "// " after leading whitespace.
            for &line in lines.iter().rev() {
                let content = self.model.get_line_content(line).to_string();
                let ws_len = content.len() - content.trim_start().len();
                let insert_col = (ws_len as u32) + 1;
                self.model.insert(Position::new(line, insert_col), "// ");
            }
        }
    }

    /// Select the entire current line (including newline).
    fn select_line(&mut self) {
        let line = self.cursors.get_primary().position().line;
        let max_col = self.model.get_line_max_column(line);
        let start = Position::new(line, 1);
        let end = if line < self.model.get_line_count() {
            Position::new(line + 1, 1)
        } else {
            Position::new(line, max_col)
        };
        self.cursors.set_state(
            0,
            CursorState {
                selection: Selection::from_positions(start, end),
            },
        );
    }

    /// Insert a blank line below the current line and move cursor there.
    fn insert_line_below(&mut self) {
        let line = self.cursors.get_primary().position().line;
        let max_col = self.model.get_line_max_column(line);
        self.model.insert(Position::new(line, max_col), "\n");
        self.cursors.set_state(
            0,
            CursorState::from_position(Position::new(line + 1, 1)),
        );
    }

    /// Insert a blank line above the current line and move cursor there.
    fn insert_line_above(&mut self) {
        let line = self.cursors.get_primary().position().line;
        self.model.insert(Position::new(line, 1), "\n");
        self.cursors.set_state(
            0,
            CursorState::from_position(Position::new(line, 1)),
        );
    }

    // -- multi-cursor: find-match ------------------------------------------

    /// Get the word under the primary cursor.
    fn word_under_cursor(&self) -> Option<(String, Range)> {
        let pos = self.cursors.get_primary().position();
        let sel = self.cursors.get_primary().selection.as_range();
        if !sel.is_empty() {
            // Already have a selection — return the selected text.
            let text = self.model.get_value_in_range(sel);
            return Some((text, sel));
        }
        let content = self.model.get_line_content(pos.line);
        let bytes = content.as_bytes();
        let col_idx = (pos.column as usize).saturating_sub(1);
        if col_idx >= bytes.len() {
            return None;
        }
        // Find word boundaries around cursor.
        let mut start = col_idx;
        let mut end = col_idx;
        while start > 0 && is_word_char(bytes[start - 1]) {
            start -= 1;
        }
        while end < bytes.len() && is_word_char(bytes[end]) {
            end += 1;
        }
        if start == end {
            return None;
        }
        let word = content[start..end].to_string();
        let range = Range::new(
            pos.line,
            (start as u32) + 1,
            pos.line,
            (end as u32) + 1,
        );
        Some((word, range))
    }

    /// Add selection to next find match (Ctrl+D behavior).
    fn add_selection_to_next_find_match(&mut self) {
        let sel = self.cursors.get_primary().selection.as_range();
        if sel.is_empty() {
            // First press: select the word under cursor.
            if let Some((_word, range)) = self.word_under_cursor() {
                self.cursors.set_state(
                    0,
                    CursorState {
                        selection: Selection::from_positions(range.start, range.end),
                    },
                );
            }
            return;
        }
        // Subsequent presses: find next occurrence and add cursor there.
        let selected_text = self.model.get_value_in_range(sel);
        let matches = self.model.find_matches(&selected_text, false, true);
        // Find the first match that starts after the last cursor's selection end.
        let all_cursors = self.cursors.get_all();
        let mut max_end = Position::new(1, 1);
        for c in all_cursors {
            let r = c.selection.as_range();
            if r.end > max_end {
                max_end = r.end;
            }
        }
        // Find next match after max_end, wrapping around.
        let next = matches
            .iter()
            .find(|m| m.start >= max_end)
            .or_else(|| matches.first());
        if let Some(m) = next {
            // Don't add if a cursor already covers this range.
            let already_exists = all_cursors.iter().any(|c| c.selection.as_range() == *m);
            if !already_exists {
                self.cursors.add_cursor(m.start);
                let idx = self.cursors.get_all().len() - 1;
                self.cursors.set_state(
                    idx,
                    CursorState {
                        selection: Selection::from_positions(m.start, m.end),
                    },
                );
            }
        }
    }

    /// Select all occurrences of the word under cursor (Ctrl+Shift+L).
    fn select_all_occurrences(&mut self) {
        let (word, _range) = match self.word_under_cursor() {
            Some(w) => w,
            None => return,
        };
        let matches = self.model.find_matches(&word, false, true);
        if matches.is_empty() {
            return;
        }
        // Set primary cursor to first match.
        self.cursors.set_state(
            0,
            CursorState {
                selection: Selection::from_positions(matches[0].start, matches[0].end),
            },
        );
        // Add cursors for remaining matches.
        for m in &matches[1..] {
            self.cursors.add_cursor(m.start);
            let idx = self.cursors.get_all().len() - 1;
            self.cursors.set_state(
                idx,
                CursorState {
                    selection: Selection::from_positions(m.start, m.end),
                },
            );
        }
    }

    // -- navigation ---------------------------------------------------------

    /// Scroll up by `lines` lines.
    fn page_up(&mut self, lines: u32) {
        self.move_cursors_vertical_n(false, true, lines);
    }

    /// Scroll down by `lines` lines.
    fn page_down(&mut self, lines: u32) {
        self.move_cursors_vertical_n(false, false, lines);
    }

    /// Apply vertical movement of `n` lines.
    fn move_cursors_vertical_n(&mut self, select: bool, up: bool, n: u32) {
        let count = self.cursors.get_all().len();
        for i in 0..count {
            let cur = self.cursors.get_all()[i].clone();
            let mem = self.cursors.get_column_memory(i);
            let (new_state, new_mem) = if up {
                vsedit_cursor::move_up(&self.model, &cur, select, n, mem)
            } else {
                vsedit_cursor::move_down(&self.model, &cur, select, n, mem)
            };
            self.cursors.set_state(i, new_state);
            self.cursors.set_column_memory(i, Some(new_mem));
        }
        self.cursors.merge_overlapping();
    }

    /// Jump to matching bracket at cursor position.
    fn jump_to_matching_bracket(&mut self) {
        let pos = self.cursors.get_primary().position();
        let content = self.model.get_line_content(pos.line);
        let col_idx = (pos.column as usize).saturating_sub(1);
        let bytes = content.as_bytes();
        if col_idx >= bytes.len() {
            return;
        }
        let ch = bytes[col_idx];
        let (target, forward) = match ch {
            b'(' => (b')', true),
            b')' => (b'(', false),
            b'[' => (b']', true),
            b']' => (b'[', false),
            b'{' => (b'}', true),
            b'}' => (b'{', false),
            _ => return,
        };
        let full_text = self.model.get_value();
        let offset = self.model.position_to_offset(pos);
        let text_bytes = full_text.as_bytes();
        let mut depth = 1i32;
        if forward {
            let mut i = offset + 1;
            while i < text_bytes.len() {
                if text_bytes[i] == ch {
                    depth += 1;
                } else if text_bytes[i] == target {
                    depth -= 1;
                    if depth == 0 {
                        let new_pos = self.model.offset_to_position(i);
                        self.cursors.set_state(0, CursorState::from_position(new_pos));
                        return;
                    }
                }
                i += 1;
            }
        } else {
            if offset == 0 {
                return;
            }
            let mut i = offset - 1;
            loop {
                if text_bytes[i] == ch {
                    depth += 1;
                } else if text_bytes[i] == target {
                    depth -= 1;
                    if depth == 0 {
                        let new_pos = self.model.offset_to_position(i);
                        self.cursors.set_state(0, CursorState::from_position(new_pos));
                        return;
                    }
                }
                if i == 0 {
                    break;
                }
                i -= 1;
            }
        }
    }

    /// Go to a specific line number.
    fn go_to_line(&mut self, line: u32) {
        let target = line.max(1).min(self.model.get_line_count());
        self.cursors.set_state(
            0,
            CursorState::from_position(Position::new(target, 1)),
        );
    }

    // -- clipboard ----------------------------------------------------------

    /// Copy selected text (or the current line if no selection) to the
    /// internal clipboard.
    fn copy(&mut self) {
        let sel = self.cursors.get_primary().selection.as_range();
        if sel.is_empty() {
            let line = self.cursors.get_primary().position().line;
            let content = self.model.get_line_content(line).to_string();
            self.clipboard = content + "\n";
        } else {
            self.clipboard = self.model.get_value_in_range(sel);
        }
    }

    /// Cut selected text (or the current line) to the internal clipboard.
    fn cut(&mut self) {
        let sel = self.cursors.get_primary().selection.as_range();
        if sel.is_empty() {
            let line = self.cursors.get_primary().position().line;
            let content = self.model.get_line_content(line).to_string();
            self.clipboard = content + "\n";
            self.delete_line();
        } else {
            self.clipboard = self.model.get_value_in_range(sel);
            self.model.delete(sel);
            self.cursors.set_state(0, CursorState::from_position(sel.start));
            self.cursors.merge_overlapping();
        }
    }

    // -- find/replace -------------------------------------------------------

    /// Populate find results for the given query.
    fn find(&mut self, query: &str) {
        let matches = self.model.find_matches(query, false, true);
        self.find_results = matches
            .iter()
            .map(|r| {
                (
                    self.model.position_to_offset(r.start),
                    self.model.position_to_offset(r.end),
                )
            })
            .collect();
        self.find_index = 0;
        // Jump to first match if any.
        if let Some(&(start, _end)) = self.find_results.first() {
            let pos = self.model.offset_to_position(start);
            self.cursors.set_state(0, CursorState::from_position(pos));
        }
    }

    /// Move to the next find result.
    fn find_next(&mut self) {
        if self.find_results.is_empty() {
            return;
        }
        self.find_index = (self.find_index + 1) % self.find_results.len();
        let (start, _end) = self.find_results[self.find_index];
        let pos = self.model.offset_to_position(start);
        self.cursors.set_state(0, CursorState::from_position(pos));
    }

    /// Move to the previous find result.
    fn find_previous(&mut self) {
        if self.find_results.is_empty() {
            return;
        }
        if self.find_index == 0 {
            self.find_index = self.find_results.len() - 1;
        } else {
            self.find_index -= 1;
        }
        let (start, _end) = self.find_results[self.find_index];
        let pos = self.model.offset_to_position(start);
        self.cursors.set_state(0, CursorState::from_position(pos));
    }

    /// Replace the current find match with replacement text.
    fn replace_current(&mut self, search: &str, replacement: &str) {
        if self.find_results.is_empty() {
            return;
        }
        let idx = self.find_index.min(self.find_results.len() - 1);
        let (start_off, end_off) = self.find_results[idx];
        let start = self.model.offset_to_position(start_off);
        let end = self.model.offset_to_position(end_off);
        self.model.apply_edit(Range::from_positions(start, end), replacement);
        // Re-run the find to refresh results.
        self.find(search);
    }

    /// Replace all find matches.
    fn replace_all(&mut self, search: &str, replacement: &str) {
        if self.find_results.is_empty() {
            self.find(search);
        }
        // Replace from bottom to top to keep offsets valid.
        let mut results = self.find_results.clone();
        results.sort_by(|a, b| b.0.cmp(&a.0));
        for (start_off, end_off) in results {
            let start = self.model.offset_to_position(start_off);
            let end = self.model.offset_to_position(end_off);
            self.model.apply_edit(Range::from_positions(start, end), replacement);
        }
        self.find_results.clear();
        self.find_index = 0;
    }

    // -- Selection operations -----------------------------------------------

    /// Select the word under the primary cursor (double-click behavior).
    pub fn select_word_at_cursor(&mut self) {
        let cursor = self.cursors.get_primary().clone();
        let new_state = vsedit_cursor::select_word_at(&self.model, &cursor);
        self.cursors.set_state(0, new_state);
    }

    /// Select a specific line number (including newline).
    pub fn select_line_number(&mut self, line_num: u32) {
        let line = line_num.max(1).min(self.model.get_line_count());
        let start = Position::new(line, 1);
        let end = if line < self.model.get_line_count() {
            Position::new(line + 1, 1)
        } else {
            Position::new(line, self.model.get_line_max_column(line))
        };
        self.cursors.set_state(
            0,
            CursorState {
                selection: Selection::from_positions(start, end),
            },
        );
    }

    /// Find all occurrences of `word` in the text and return their ranges.
    pub fn find_all_occurrences(&self, word: &str) -> Vec<Range> {
        self.model.find_matches(word, false, true)
    }

    /// Create a column (box/rectangular) selection from start to end positions.
    /// This creates one cursor per line in the rectangle.
    pub fn column_selection(&mut self, start: Position, end: Position) {
        let start_line = start.line.min(end.line);
        let end_line = start.line.max(end.line);
        let start_col = start.column.min(end.column);
        let end_col = start.column.max(end.column);

        let line_count = self.model.get_line_count();
        let mut first = true;
        for line in start_line..=end_line.min(line_count) {
            let max_col = self.model.get_line_max_column(line);
            let sc = start_col.min(max_col);
            let ec = end_col.min(max_col);
            let state = CursorState {
                selection: Selection::from_positions(
                    Position::new(line, sc),
                    Position::new(line, ec),
                ),
            };
            if first {
                self.cursors.set_state(0, state);
                first = false;
            } else {
                self.cursors.add_cursor(Position::new(line, ec));
                let idx = self.cursors.get_all().len() - 1;
                self.cursors.set_state(idx, state);
            }
        }
    }
}

fn is_word_char(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
}

impl Default for EditorController {
    fn default() -> Self {
        Self::new("")
    }
}

// ---------------------------------------------------------------------------
// EditorAction — utility methods
// ---------------------------------------------------------------------------

impl EditorAction {
    /// Returns `true` if this action modifies the text buffer.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            EditorAction::InsertText(_)
                | EditorAction::DeleteLeft
                | EditorAction::DeleteRight
                | EditorAction::DeleteWordLeft
                | EditorAction::DeleteWordRight
                | EditorAction::DeleteLine
                | EditorAction::NewLine
                | EditorAction::IndentLine
                | EditorAction::OutdentLine
                | EditorAction::MoveLineUp
                | EditorAction::MoveLineDown
                | EditorAction::ToggleLineComment
                | EditorAction::InsertLineBelow
                | EditorAction::InsertLineAbove
                | EditorAction::Cut
                | EditorAction::Paste(_)
                | EditorAction::Replace(_, _)
                | EditorAction::ReplaceAll(_, _)
                | EditorAction::Undo
                | EditorAction::Redo
        )
    }

    /// Returns `true` if this action only moves the cursor without selecting.
    pub fn is_cursor_movement(&self) -> bool {
        matches!(
            self,
            EditorAction::MoveCursorLeft
                | EditorAction::MoveCursorRight
                | EditorAction::MoveCursorUp
                | EditorAction::MoveCursorDown
                | EditorAction::MoveCursorWordLeft
                | EditorAction::MoveCursorWordRight
                | EditorAction::MoveCursorLineStart
                | EditorAction::MoveCursorLineEnd
                | EditorAction::MoveCursorDocumentStart
                | EditorAction::MoveCursorDocumentEnd
                | EditorAction::PageUp(_)
                | EditorAction::PageDown(_)
                | EditorAction::GoToLine(_)
                | EditorAction::JumpToMatchingBracket
        )
    }

    /// Returns `true` if this action extends the selection.
    pub fn is_selection(&self) -> bool {
        matches!(
            self,
            EditorAction::SelectLeft
                | EditorAction::SelectRight
                | EditorAction::SelectUp
                | EditorAction::SelectDown
                | EditorAction::SelectAll
                | EditorAction::SelectLine
                | EditorAction::AddSelectionToNextFindMatch
                | EditorAction::SelectAllOccurrences
        )
    }

    /// Returns a short human-readable name for the action.
    pub fn name(&self) -> &'static str {
        match self {
            EditorAction::MoveCursorLeft => "MoveCursorLeft",
            EditorAction::MoveCursorRight => "MoveCursorRight",
            EditorAction::MoveCursorUp => "MoveCursorUp",
            EditorAction::MoveCursorDown => "MoveCursorDown",
            EditorAction::MoveCursorWordLeft => "MoveCursorWordLeft",
            EditorAction::MoveCursorWordRight => "MoveCursorWordRight",
            EditorAction::MoveCursorLineStart => "MoveCursorLineStart",
            EditorAction::MoveCursorLineEnd => "MoveCursorLineEnd",
            EditorAction::MoveCursorDocumentStart => "MoveCursorDocumentStart",
            EditorAction::MoveCursorDocumentEnd => "MoveCursorDocumentEnd",
            EditorAction::SelectLeft => "SelectLeft",
            EditorAction::SelectRight => "SelectRight",
            EditorAction::SelectUp => "SelectUp",
            EditorAction::SelectDown => "SelectDown",
            EditorAction::SelectAll => "SelectAll",
            EditorAction::InsertText(_) => "InsertText",
            EditorAction::DeleteLeft => "DeleteLeft",
            EditorAction::DeleteRight => "DeleteRight",
            EditorAction::DeleteWordLeft => "DeleteWordLeft",
            EditorAction::DeleteWordRight => "DeleteWordRight",
            EditorAction::DeleteLine => "DeleteLine",
            EditorAction::NewLine => "NewLine",
            EditorAction::IndentLine => "IndentLine",
            EditorAction::OutdentLine => "OutdentLine",
            EditorAction::MoveLineUp => "MoveLineUp",
            EditorAction::MoveLineDown => "MoveLineDown",
            EditorAction::ToggleLineComment => "ToggleLineComment",
            EditorAction::SelectLine => "SelectLine",
            EditorAction::InsertLineBelow => "InsertLineBelow",
            EditorAction::InsertLineAbove => "InsertLineAbove",
            EditorAction::AddCursorAbove => "AddCursorAbove",
            EditorAction::AddCursorBelow => "AddCursorBelow",
            EditorAction::AddSelectionToNextFindMatch => "AddSelectionToNextFindMatch",
            EditorAction::SelectAllOccurrences => "SelectAllOccurrences",
            EditorAction::CursorUndo => "CursorUndo",
            EditorAction::RemoveSecondaryCursors => "RemoveSecondaryCursors",
            EditorAction::PageUp(_) => "PageUp",
            EditorAction::PageDown(_) => "PageDown",
            EditorAction::JumpToMatchingBracket => "JumpToMatchingBracket",
            EditorAction::GoToLine(_) => "GoToLine",
            EditorAction::Copy => "Copy",
            EditorAction::Cut => "Cut",
            EditorAction::Paste(_) => "Paste",
            EditorAction::Find(_) => "Find",
            EditorAction::FindNext => "FindNext",
            EditorAction::FindPrevious => "FindPrevious",
            EditorAction::Replace(_, _) => "Replace",
            EditorAction::ReplaceAll(_, _) => "ReplaceAll",
            EditorAction::Undo => "Undo",
            EditorAction::Redo => "Redo",
            EditorAction::ToggleAutoClose => "ToggleAutoClose",
        }
    }
}

// ---------------------------------------------------------------------------
// EditorController — query / inspection helpers
// ---------------------------------------------------------------------------

impl EditorController {
    /// Execute a sequence of actions in order.
    pub fn execute_actions(&mut self, actions: &[EditorAction]) {
        for action in actions {
            self.execute_action(action.clone());
        }
    }

    /// Return the full text content of the model.
    pub fn text(&self) -> String {
        self.model.get_value()
    }

    /// Return the number of lines in the document.
    pub fn line_count(&self) -> u32 {
        self.model.get_line_count()
    }

    /// Return the content of a specific line (1-based).
    pub fn line_content(&self, line: u32) -> &str {
        self.model.get_line_content(line)
    }

    /// Return the current primary cursor position.
    pub fn cursor_position(&self) -> Position {
        self.cursors.get_primary().position()
    }

    /// Return the current primary selection as a `Range`.
    pub fn selection_range(&self) -> Range {
        self.cursors.get_primary().selection.as_range()
    }

    /// Return the text currently selected by the primary cursor, or an empty
    /// string if no selection exists.
    pub fn selected_text(&self) -> String {
        let range = self.selection_range();
        if range.is_empty() {
            String::new()
        } else {
            self.model.get_value_in_range(range)
        }
    }

    /// Returns `true` when the primary cursor has a non-empty selection.
    pub fn has_selection(&self) -> bool {
        !self.selection_range().is_empty()
    }

    /// Returns `true` when there is more than one cursor active.
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.has_multiple_cursors()
    }

    /// Return the number of active cursors.
    pub fn cursor_count(&self) -> usize {
        self.cursors.cursor_count()
    }

    /// Returns `true` when the document is empty.
    pub fn is_empty(&self) -> bool {
        self.model.is_empty()
    }

    /// Returns the number of words in the document.
    pub fn word_count(&self) -> usize {
        self.model.get_word_count()
    }

    /// Returns the number of characters in the document.
    pub fn char_count(&self) -> usize {
        self.model.get_char_count()
    }

    /// Returns true if the cursor is on the first line.
    pub fn cursor_at_first_line(&self) -> bool {
        self.cursor_position().line == 1
    }

    /// Returns true if the cursor is on the last line.
    pub fn cursor_at_last_line(&self) -> bool {
        self.cursor_position().line == self.line_count()
    }

    /// Returns true if the cursor is at the very start of the document.
    pub fn cursor_at_document_start(&self) -> bool {
        let pos = self.cursor_position();
        pos.line == 1 && pos.column == 1
    }

    /// Returns true if the cursor is at the very end of the document.
    pub fn cursor_at_document_end(&self) -> bool {
        let pos = self.cursor_position();
        let last_line = self.line_count();
        pos.line == last_line && pos.column == self.model.get_line_max_column(last_line)
    }

    /// Returns the content of the line the primary cursor is on.
    pub fn current_line_content(&self) -> &str {
        self.model.get_line_content(self.cursor_position().line)
    }

    /// Returns the length of the line the primary cursor is on.
    pub fn current_line_length(&self) -> u32 {
        self.model.get_line_length(self.cursor_position().line)
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
// MacroAction / ControllerMacroRecorder
// ---------------------------------------------------------------------------

/// An individual action that can be recorded and replayed by the macro system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacroAction {
    Insert(String),
    Delete(usize),
    MoveCursor(i32, i32),
    Select(usize, usize),
}

impl std::fmt::Display for MacroAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacroAction::Insert(s) => write!(f, "Insert({s:?})"),
            MacroAction::Delete(n) => write!(f, "Delete({n})"),
            MacroAction::MoveCursor(dx, dy) => write!(f, "MoveCursor({dx}, {dy})"),
            MacroAction::Select(start, end) => write!(f, "Select({start}, {end})"),
        }
    }
}

/// Records and replays keystroke sequences for macro functionality.
pub struct ControllerMacroRecorder {
    recording: bool,
    actions: Vec<MacroAction>,
}

impl ControllerMacroRecorder {
    pub fn new() -> Self {
        Self {
            recording: false,
            actions: Vec::new(),
        }
    }

    pub fn start_recording(&mut self) {
        self.recording = true;
        self.actions.clear();
    }

    pub fn stop_recording(&mut self) -> Vec<MacroAction> {
        self.recording = false;
        std::mem::take(&mut self.actions)
    }

    pub fn record_action(&mut self, action: MacroAction) {
        if self.recording {
            self.actions.push(action);
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }
}

impl Default for ControllerMacroRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Gesture / ClickType / ControllerGestureRecognizer
// ---------------------------------------------------------------------------

/// A mouse gesture recognised from a down-move-up sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gesture {
    Select,
    Drag { from: (u16, u16), to: (u16, u16) },
    None,
}

impl std::fmt::Display for Gesture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Gesture::Select => write!(f, "Select"),
            Gesture::Drag { from, to } => {
                write!(f, "Drag({},{} -> {},{})", from.0, from.1, to.0, to.1)
            }
            Gesture::None => write!(f, "None"),
        }
    }
}

/// The click multiplicity detected by the gesture recognizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickType {
    Single,
    Double,
    Triple,
}

impl std::fmt::Display for ClickType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            ClickType::Single => "Single",
            ClickType::Double => "Double",
            ClickType::Triple => "Triple",
        };
        f.write_str(label)
    }
}

/// Recognises mouse gestures from raw pointer events.
pub struct ControllerGestureRecognizer {
    down_pos: Option<(u16, u16)>,
    last_pos: Option<(u16, u16)>,
    moved: bool,
}

impl ControllerGestureRecognizer {
    pub fn new() -> Self {
        Self {
            down_pos: None,
            last_pos: None,
            moved: false,
        }
    }

    pub fn on_mouse_down(&mut self, x: u16, y: u16) {
        self.down_pos = Some((x, y));
        self.last_pos = Some((x, y));
        self.moved = false;
    }

    pub fn on_mouse_move(&mut self, x: u16, y: u16) {
        if let Some(down) = self.down_pos {
            if (x, y) != down {
                self.moved = true;
            }
        }
        self.last_pos = Some((x, y));
    }

    pub fn on_mouse_up(&mut self) -> Option<Gesture> {
        let result = match (self.down_pos, self.last_pos, self.moved) {
            (Some(from), Some(to), true) => Some(Gesture::Drag { from, to }),
            (Some(_), Some(_), false) => Some(Gesture::Select),
            _ => Some(Gesture::None),
        };
        self.down_pos = None;
        self.last_pos = None;
        self.moved = false;
        result
    }

    pub fn on_click(&mut self, x: u16, y: u16, count: u8) -> ClickType {
        self.on_mouse_down(x, y);
        match count {
            2 => ClickType::Double,
            n if n >= 3 => ClickType::Triple,
            _ => ClickType::Single,
        }
    }
}

impl Default for ControllerGestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditKind / EditAction / ControllerUndoGrouping
// ---------------------------------------------------------------------------

/// The kind of mutation an [`EditAction`] represents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
    Replace,
}

impl std::fmt::Display for EditKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            EditKind::Insert => "Insert",
            EditKind::Delete => "Delete",
            EditKind::Replace => "Replace",
        };
        f.write_str(label)
    }
}

/// A single edit that can be grouped for undo purposes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditAction {
    pub kind: EditKind,
    pub text: String,
    pub position: (usize, usize),
}

impl std::fmt::Display for EditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({:?} @ {},{})",
            self.kind, self.text, self.position.0, self.position.1
        )
    }
}

/// An undo group: a labelled collection of edits treated as one undo step.
struct UndoGroup {
    label: String,
    edits: Vec<EditAction>,
}

/// Groups sequential edits so they can be undone as a single operation.
pub struct ControllerUndoGrouping {
    groups: Vec<UndoGroup>,
    active_label: Option<String>,
}

impl ControllerUndoGrouping {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            active_label: None,
        }
    }

    pub fn begin_group(&mut self, label: &str) {
        self.active_label = Some(label.to_string());
        self.groups.push(UndoGroup {
            label: label.to_string(),
            edits: Vec::new(),
        });
    }

    pub fn end_group(&mut self) {
        self.active_label = None;
    }

    pub fn add_edit(&mut self, edit: EditAction) {
        if let Some(group) = self.groups.last_mut() {
            if self.active_label.is_some() {
                group.edits.push(edit);
            }
        }
    }

    pub fn current_group(&self) -> Option<&str> {
        self.active_label.as_deref()
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Returns the label of the most recent group, if any.
    pub fn last_group_label(&self) -> Option<&str> {
        self.groups.last().map(|g| g.label.as_str())
    }

    pub fn is_grouping(&self) -> bool {
        self.active_label.is_some()
    }
}

impl Default for ControllerUndoGrouping {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ControllerInputDebouncer
// ---------------------------------------------------------------------------

/// Debounces rapid input events so that only one is processed per threshold
/// window.
pub struct ControllerInputDebouncer {
    threshold_ms: u64,
    last_ts: Option<u64>,
    dropped: usize,
}

impl ControllerInputDebouncer {
    pub fn new(threshold_ms: u64) -> Self {
        Self {
            threshold_ms,
            last_ts: None,
            dropped: 0,
        }
    }

    /// Returns `true` when enough time has elapsed since the last processed
    /// event; otherwise the event is silently dropped.
    pub fn should_process(&mut self, timestamp_ms: u64) -> bool {
        match self.last_ts {
            Some(prev) if timestamp_ms.saturating_sub(prev) < self.threshold_ms => {
                self.dropped += 1;
                false
            }
            _ => {
                self.last_ts = Some(timestamp_ms);
                true
            }
        }
    }

    pub fn last_processed(&self) -> Option<u64> {
        self.last_ts
    }

    pub fn events_dropped(&self) -> usize {
        self.dropped
    }

    pub fn reset(&mut self) {
        self.last_ts = None;
        self.dropped = 0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ─── EdBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for editor commands.
#[derive(Debug, Clone)]
pub struct EdBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> EdBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for EdBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EdBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── EdC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for editor state.
#[derive(Debug)]
pub struct EdCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> EdCLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for EdCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EdCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}


/// Configuration manager for editor_controller functionality.
pub struct EditorControllerConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl EditorControllerConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &EditorControllerConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for editor_controller operations.
pub struct EditorControllerRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl EditorControllerRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for editor_controller.
pub struct EditorControllerValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl EditorControllerValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &EditorControllerValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Editor input and cursor control — extended utilities (yh)
// ---------------------------------------------------------------------------

/// Metric accumulator for editor_ctrl operations.
#[derive(Debug, Clone)]
pub struct YhMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YhMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for editor_ctrl.
#[derive(Debug, Clone)]
pub struct YhRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YhRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for editor_ctrl lookups.
#[derive(Debug, Clone)]
pub struct YhLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YhLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for editor_controller
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaEditorControllerRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaEditorControllerRingBuf {
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
pub struct XaEditorControllerCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaEditorControllerCounter {
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

impl Default for XaEditorControllerCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 35
// ---------------------------------------------------------------------------

/// Generic object pool `Xc35Pool<T>`.
pub struct Xc35Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc35Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc35PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc35Pool<T> {
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
    pub fn stats(&self) -> Xc35PoolStats {
        Xc35PoolStats {
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

impl<T> Default for Xc35Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc35Scheduler`.
pub struct Xc35Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc35Scheduler {
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

impl Default for Xc35Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_35 hash for the given byte slice.
pub fn xc_35_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_35 convention.
pub fn xc_35_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_85 deepening: state machine + event bus ---

/// States for the Xd85 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd85State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd85State {
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
pub struct Xd85Transition {
    pub from: Xd85State,
    pub to: Xd85State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd85StateMachine {
    current: Xd85State,
    history: Vec<Xd85Transition>,
    step_counter: usize,
}

impl Xd85StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd85State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd85State {
        self.current
    }

    pub fn history(&self) -> &[Xd85Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd85State) -> Result<Xd85State, String> {
        let allowed = match (self.current, target) {
            (Xd85State::Idle, Xd85State::Running) => true,
            (Xd85State::Running, Xd85State::Paused) => true,
            (Xd85State::Running, Xd85State::Done) => true,
            (Xd85State::Paused, Xd85State::Running) => true,
            (Xd85State::Paused, Xd85State::Done) => true,
            (Xd85State::Done, Xd85State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_85: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd85Transition {
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
            "Xd85SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd85State> {
        let prefix = "Xd85SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd85State::Idle),
            "Running" => Some(Xd85State::Running),
            "Paused" => Some(Xd85State::Paused),
            "Done" => Some(Xd85State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd85State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd85 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd85Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd85Event {
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

type Xd85HandlerFn = Box<dyn Fn(&Xd85Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd85EventBus {
    handlers: Vec<(usize, Option<String>, Xd85HandlerFn)>,
    next_id: usize,
    published: Vec<Xd85Event>,
}

impl Xd85EventBus {
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
        F: Fn(&Xd85Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd85Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd85Event) {
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

    pub fn published_events(&self) -> &[Xd85Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #106
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf106Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf106TrieNode {
    children: std::collections::HashMap<char, Xf106TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf106Trie {
    root: Xf106TrieNode,
    count: usize,
}

impl Xf106Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf106TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf106TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf106TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf106BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf106BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 34).
pub struct Xh34SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh34SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 76 as u64,
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

/// A compact bit set supporting boolean operations (variant 34).
pub struct Xh34BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh34BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 34).
pub struct Xi34Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi34Deque<T> {
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
pub struct Xi34Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi34Interval {
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

/// A simple interval tree (variant 34).
pub struct Xi34IntervalTree {
    xi_intervals: Vec<Xi34Interval>,
}

impl Xi34IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi34Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi34Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi34Interval) -> Vec<&Xi34Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi34Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi34Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi34Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi34Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi34Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi34Interval> = Vec::new();
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

    // -- New editor operation tests -----------------------------------------

    #[test]
    fn select_word_at_cursor_selects_word() {
        let mut c = ctrl("hello world");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 3)));
        c.select_word_at_cursor();
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.anchor, Position::new(1, 1));
        assert_eq!(sel.active, Position::new(1, 6));
    }

    #[test]
    fn select_line_number_basic() {
        let mut c = ctrl("hello\nworld\nfoo");
        c.select_line_number(2);
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.anchor, Position::new(2, 1));
        assert_eq!(sel.active, Position::new(3, 1));
    }

    #[test]
    fn select_line_number_last_line() {
        let mut c = ctrl("hello\nworld");
        c.select_line_number(2);
        let sel = c.cursors.get_primary().selection;
        assert_eq!(sel.anchor, Position::new(2, 1));
        assert_eq!(sel.active, Position::new(2, 6));
    }

    #[test]
    fn find_all_occurrences_basic() {
        let c = ctrl("hello world hello");
        let results = c.find_all_occurrences("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn column_selection_creates_multi_cursor() {
        let mut c = ctrl("hello\nworld\nfoo!!");
        c.column_selection(Position::new(1, 1), Position::new(3, 3));
        assert_eq!(c.cursors.get_all().len(), 3);
        // Each cursor selects columns 1-3 on its line
        assert_eq!(c.cursors.get_all()[0].selection.anchor, Position::new(1, 1));
        assert_eq!(c.cursors.get_all()[0].selection.active, Position::new(1, 3));
    }

    #[test]
    fn jump_to_matching_bracket_action() {
        let mut c = ctrl("fn f() { x }");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 8)));
        c.execute_action(EditorAction::JumpToMatchingBracket);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 12));
    }

    #[test]
    fn go_to_line_action() {
        let mut c = ctrl("line1\nline2\nline3");
        c.execute_action(EditorAction::GoToLine(3));
        assert_eq!(c.cursors.get_primary().position(), Position::new(3, 1));
    }

    // -- clipboard tests ---------------------------------------------------

    #[test]
    fn copy_with_selection() {
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
        c.execute_action(EditorAction::Copy);
        assert_eq!(c.clipboard, "hello");
        assert_eq!(c.model.get_value(), "hello world");
    }

    #[test]
    fn copy_no_selection_copies_line() {
        let mut c = ctrl("hello\nworld");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 3)));
        c.execute_action(EditorAction::Copy);
        assert_eq!(c.clipboard, "hello\n");
        assert_eq!(c.model.get_value(), "hello\nworld");
    }

    #[test]
    fn cut_with_selection() {
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
        c.execute_action(EditorAction::Cut);
        assert_eq!(c.clipboard, "hello");
        assert_eq!(c.model.get_value(), " world");
    }

    #[test]
    fn cut_no_selection_cuts_line() {
        let mut c = ctrl("hello\nworld");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 3)));
        c.execute_action(EditorAction::Cut);
        assert_eq!(c.clipboard, "hello\n");
        assert_eq!(c.model.get_value(), "world");
    }

    #[test]
    fn paste_inserts_text() {
        let mut c = ctrl("hello");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 6)));
        c.execute_action(EditorAction::Paste(" world".into()));
        assert_eq!(c.model.get_value(), "hello world");
    }

    // -- find/replace tests ------------------------------------------------

    #[test]
    fn find_populates_results() {
        let mut c = ctrl("hello world hello");
        c.execute_action(EditorAction::Find("hello".into()));
        assert_eq!(c.find_results.len(), 2);
        assert_eq!(c.find_index, 0);
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 1));
    }

    #[test]
    fn find_next_cycles() {
        let mut c = ctrl("aa bb aa");
        c.execute_action(EditorAction::Find("aa".into()));
        assert_eq!(c.find_results.len(), 2);
        c.execute_action(EditorAction::FindNext);
        assert_eq!(c.find_index, 1);
        c.execute_action(EditorAction::FindNext);
        assert_eq!(c.find_index, 0);
    }

    #[test]
    fn find_previous_cycles() {
        let mut c = ctrl("aa bb aa");
        c.execute_action(EditorAction::Find("aa".into()));
        c.execute_action(EditorAction::FindPrevious);
        assert_eq!(c.find_index, 1);
    }

    #[test]
    fn replace_current_match() {
        let mut c = ctrl("hello world hello");
        c.execute_action(EditorAction::Find("hello".into()));
        c.execute_action(EditorAction::Replace("hello".into(), "hi".into()));
        assert_eq!(c.model.get_value(), "hi world hello");
    }

    #[test]
    fn replace_all_matches() {
        let mut c = ctrl("hello world hello");
        c.execute_action(EditorAction::Find("hello".into()));
        c.execute_action(EditorAction::ReplaceAll("hello".into(), "hi".into()));
        assert_eq!(c.model.get_value(), "hi world hi");
        assert!(c.find_results.is_empty());
    }

    // -- auto-close pair tests ----------------------------------------------

    #[test]
    fn auto_close_paren() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("(".into()));
        assert_eq!(c.model.get_value(), "()");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    #[test]
    fn auto_close_bracket() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("[".into()));
        assert_eq!(c.model.get_value(), "[]");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    #[test]
    fn auto_close_brace() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("{".into()));
        assert_eq!(c.model.get_value(), "{}");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    #[test]
    fn auto_close_double_quote() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("\"".into()));
        assert_eq!(c.model.get_value(), "\"\"");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    #[test]
    fn auto_close_backtick() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("`".into()));
        assert_eq!(c.model.get_value(), "``");
        assert_eq!(c.cursors.get_primary().position(), Position::new(1, 2));
    }

    #[test]
    fn auto_close_delete_pair() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("(".into()));
        assert_eq!(c.model.get_value(), "()");
        // Cursor is between ( and )
        c.execute_action(EditorAction::DeleteLeft);
        assert_eq!(c.model.get_value(), "");
    }

    #[test]
    fn auto_close_delete_brace_pair() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("{".into()));
        assert_eq!(c.model.get_value(), "{}");
        c.execute_action(EditorAction::DeleteLeft);
        assert_eq!(c.model.get_value(), "");
    }

    #[test]
    fn auto_close_no_trigger_on_multichar() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("ab".into()));
        assert_eq!(c.model.get_value(), "ab");
    }

    #[test]
    fn auto_close_no_trigger_on_non_pair_char() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::InsertText("a".into()));
        assert_eq!(c.model.get_value(), "a");
    }

    #[test]
    fn auto_close_toggle_disables() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::ToggleAutoClose);
        assert!(!c.auto_close_enabled);
        c.execute_action(EditorAction::InsertText("(".into()));
        assert_eq!(c.model.get_value(), "(");
    }

    #[test]
    fn auto_close_toggle_re_enables() {
        let mut c = ctrl("");
        c.execute_action(EditorAction::ToggleAutoClose);
        c.execute_action(EditorAction::ToggleAutoClose);
        assert!(c.auto_close_enabled);
        c.execute_action(EditorAction::InsertText("(".into()));
        assert_eq!(c.model.get_value(), "()");
    }

    #[test]
    fn auto_close_normal_backspace_not_between_pair() {
        let mut c = ctrl("abc");
        c.cursors.set_state(0, CursorState::from_position(Position::new(1, 3)));
        c.execute_action(EditorAction::DeleteLeft);
        assert_eq!(c.model.get_value(), "ac");
    }

    // -----------------------------------------------------------------------
    // EditorAction predicate / utility tests
    // -----------------------------------------------------------------------

    #[test]
    fn action_is_mutating_insert() {
        assert!(EditorAction::InsertText("x".into()).is_mutating());
        assert!(EditorAction::DeleteLeft.is_mutating());
        assert!(EditorAction::NewLine.is_mutating());
        assert!(EditorAction::Cut.is_mutating());
        assert!(EditorAction::Paste("t".into()).is_mutating());
        assert!(EditorAction::Replace("a".into(), "b".into()).is_mutating());
        assert!(EditorAction::Undo.is_mutating());
        assert!(EditorAction::Redo.is_mutating());
    }

    #[test]
    fn action_is_not_mutating_movement() {
        assert!(!EditorAction::MoveCursorLeft.is_mutating());
        assert!(!EditorAction::MoveCursorRight.is_mutating());
        assert!(!EditorAction::SelectAll.is_mutating());
        assert!(!EditorAction::Copy.is_mutating());
        assert!(!EditorAction::Find("x".into()).is_mutating());
        assert!(!EditorAction::GoToLine(1).is_mutating());
    }

    #[test]
    fn action_is_cursor_movement() {
        assert!(EditorAction::MoveCursorLeft.is_cursor_movement());
        assert!(EditorAction::MoveCursorDocumentEnd.is_cursor_movement());
        assert!(EditorAction::PageUp(10).is_cursor_movement());
        assert!(EditorAction::GoToLine(5).is_cursor_movement());
        assert!(EditorAction::JumpToMatchingBracket.is_cursor_movement());
        assert!(!EditorAction::SelectAll.is_cursor_movement());
        assert!(!EditorAction::InsertText("a".into()).is_cursor_movement());
    }

    #[test]
    fn action_is_selection() {
        assert!(EditorAction::SelectLeft.is_selection());
        assert!(EditorAction::SelectRight.is_selection());
        assert!(EditorAction::SelectUp.is_selection());
        assert!(EditorAction::SelectDown.is_selection());
        assert!(EditorAction::SelectAll.is_selection());
        assert!(EditorAction::SelectLine.is_selection());
        assert!(!EditorAction::MoveCursorLeft.is_selection());
        assert!(!EditorAction::InsertText("a".into()).is_selection());
    }

    #[test]
    fn action_name_returns_string() {
        assert_eq!(EditorAction::MoveCursorLeft.name(), "MoveCursorLeft");
        assert_eq!(EditorAction::InsertText("x".into()).name(), "InsertText");
        assert_eq!(EditorAction::DeleteLine.name(), "DeleteLine");
        assert_eq!(EditorAction::Paste("p".into()).name(), "Paste");
        assert_eq!(EditorAction::Undo.name(), "Undo");
        assert_eq!(EditorAction::ToggleAutoClose.name(), "ToggleAutoClose");
    }

    #[test]
    fn action_categories_are_mutually_exclusive() {
        // Movement actions should not be selections or mutations
        let movements = [
            EditorAction::MoveCursorLeft,
            EditorAction::MoveCursorRight,
            EditorAction::MoveCursorUp,
            EditorAction::MoveCursorDown,
        ];
        for a in &movements {
            assert!(a.is_cursor_movement());
            assert!(!a.is_selection());
            assert!(!a.is_mutating());
        }
    }

    // -----------------------------------------------------------------------
    // EditorController query / inspection tests
    // -----------------------------------------------------------------------

    #[test]
    fn controller_text_returns_content() {
        let c = ctrl("hello world");
        assert_eq!(c.text(), "hello world");
    }

    #[test]
    fn controller_line_count() {
        let c = ctrl("a\nb\nc");
        assert_eq!(c.line_count(), 3);
    }

    #[test]
    fn controller_line_content() {
        let c = ctrl("alpha\nbeta\ngamma");
        assert_eq!(c.line_content(1), "alpha");
        assert_eq!(c.line_content(2), "beta");
        assert_eq!(c.line_content(3), "gamma");
    }

    #[test]
    fn controller_cursor_position_after_new() {
        let c = ctrl("hello");
        assert_eq!(c.cursor_position(), Position::new(1, 1));
    }

    #[test]
    fn controller_selected_text_empty_when_no_selection() {
        let c = ctrl("hello");
        assert_eq!(c.selected_text(), "");
        assert!(!c.has_selection());
    }

    #[test]
    fn controller_selected_text_with_selection() {
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
        assert_eq!(c.selected_text(), "hello");
        assert!(c.has_selection());
    }

    #[test]
    fn controller_is_empty() {
        assert!(ctrl("").is_empty());
        assert!(!ctrl("x").is_empty());
    }

    #[test]
    fn controller_word_count() {
        let c = ctrl("hello beautiful world");
        assert_eq!(c.word_count(), 3);
    }

    #[test]
    fn controller_char_count() {
        let c = ctrl("abc");
        assert_eq!(c.char_count(), 3);
    }

    #[test]
    fn controller_cursor_at_document_start() {
        let c = ctrl("hello");
        assert!(c.cursor_at_document_start());
        assert!(c.cursor_at_first_line());
    }

    #[test]
    fn controller_cursor_at_document_end() {
        let mut c = ctrl("hello");
        c.execute_action(EditorAction::MoveCursorDocumentEnd);
        assert!(c.cursor_at_document_end());
        assert!(c.cursor_at_last_line());
    }

    #[test]
    fn controller_cursor_at_first_last_line_multiline() {
        let mut c = ctrl("a\nb\nc");
        assert!(c.cursor_at_first_line());
        assert!(!c.cursor_at_last_line());
        c.execute_action(EditorAction::GoToLine(3));
        assert!(!c.cursor_at_first_line());
        assert!(c.cursor_at_last_line());
    }

    #[test]
    fn controller_current_line_content() {
        let mut c = ctrl("hello\nworld");
        assert_eq!(c.current_line_content(), "hello");
        c.execute_action(EditorAction::MoveCursorDown);
        assert_eq!(c.current_line_content(), "world");
    }

    #[test]
    fn controller_current_line_length() {
        let c = ctrl("hello");
        assert_eq!(c.current_line_length(), 5);
    }

    #[test]
    fn controller_has_multiple_cursors() {
        let mut c = ctrl("hello\nworld");
        assert!(!c.has_multiple_cursors());
        assert_eq!(c.cursor_count(), 1);
        c.cursors.add_cursor(Position::new(2, 1));
        assert!(c.has_multiple_cursors());
        assert_eq!(c.cursor_count(), 2);
    }

    #[test]
    fn controller_execute_actions_batch() {
        let mut c = ctrl("");
        c.execute_actions(&[
            EditorAction::InsertText("hello".into()),
            EditorAction::NewLine,
            EditorAction::InsertText("world".into()),
        ]);
        assert_eq!(c.text(), "hello\nworld");
        assert_eq!(c.line_count(), 2);
    }

    #[test]
    fn controller_selection_range_after_select_all() {
        let mut c = ctrl("ab\ncd");
        c.execute_action(EditorAction::SelectAll);
        let range = c.selection_range();
        assert_eq!(range.start, Position::new(1, 1));
        assert_eq!(range.end, Position::new(2, 3));
    }

    // --- ControllerMacroRecorder tests ---

    #[test]
    fn macro_recorder_starts_not_recording() {
        let rec = ControllerMacroRecorder::new();
        assert!(!rec.is_recording());
        assert_eq!(rec.action_count(), 0);
    }

    #[test]
    fn macro_recorder_records_actions() {
        let mut rec = ControllerMacroRecorder::new();
        rec.start_recording();
        assert!(rec.is_recording());
        rec.record_action(MacroAction::Insert("hi".into()));
        rec.record_action(MacroAction::Delete(3));
        rec.record_action(MacroAction::MoveCursor(1, -1));
        assert_eq!(rec.action_count(), 3);
        let actions = rec.stop_recording();
        assert!(!rec.is_recording());
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0], MacroAction::Insert("hi".into()));
        assert_eq!(actions[2], MacroAction::MoveCursor(1, -1));
    }

    #[test]
    fn macro_recorder_ignores_when_not_recording() {
        let mut rec = ControllerMacroRecorder::new();
        rec.record_action(MacroAction::Delete(1));
        assert_eq!(rec.action_count(), 0);
    }

    #[test]
    fn macro_action_display() {
        assert_eq!(MacroAction::Select(0, 5).to_string(), "Select(0, 5)");
        assert_eq!(MacroAction::Delete(2).to_string(), "Delete(2)");
    }

    // --- ControllerGestureRecognizer tests ---

    #[test]
    fn gesture_recognizer_select() {
        let mut g = ControllerGestureRecognizer::new();
        g.on_mouse_down(10, 20);
        let gesture = g.on_mouse_up();
        assert_eq!(gesture, Some(Gesture::Select));
    }

    #[test]
    fn gesture_recognizer_drag() {
        let mut g = ControllerGestureRecognizer::new();
        g.on_mouse_down(5, 5);
        g.on_mouse_move(15, 25);
        let gesture = g.on_mouse_up();
        assert_eq!(
            gesture,
            Some(Gesture::Drag {
                from: (5, 5),
                to: (15, 25)
            })
        );
    }

    #[test]
    fn gesture_click_types() {
        let mut g = ControllerGestureRecognizer::new();
        assert_eq!(g.on_click(1, 1, 1), ClickType::Single);
        assert_eq!(g.on_click(1, 1, 2), ClickType::Double);
        assert_eq!(g.on_click(1, 1, 3), ClickType::Triple);
    }

    #[test]
    fn gesture_display() {
        assert_eq!(Gesture::None.to_string(), "None");
        assert_eq!(ClickType::Double.to_string(), "Double");
    }

    // --- ControllerUndoGrouping tests ---

    #[test]
    fn undo_grouping_basic() {
        let mut ug = ControllerUndoGrouping::new();
        assert!(!ug.is_grouping());
        assert_eq!(ug.group_count(), 0);
        ug.begin_group("typing");
        assert!(ug.is_grouping());
        assert_eq!(ug.current_group(), Some("typing"));
        ug.add_edit(EditAction {
            kind: EditKind::Insert,
            text: "a".into(),
            position: (1, 1),
        });
        ug.end_group();
        assert!(!ug.is_grouping());
        assert_eq!(ug.group_count(), 1);
    }

    #[test]
    fn edit_action_display() {
        let ea = EditAction {
            kind: EditKind::Replace,
            text: "x".into(),
            position: (3, 7),
        };
        assert_eq!(ea.to_string(), "Replace(\"x\" @ 3,7)");
    }

    // --- ControllerInputDebouncer tests ---

    #[test]
    fn debouncer_processes_first_event() {
        let mut d = ControllerInputDebouncer::new(50);
        assert!(d.should_process(100));
        assert_eq!(d.last_processed(), Some(100));
        assert_eq!(d.events_dropped(), 0);
    }

    #[test]
    fn debouncer_drops_rapid_events() {
        let mut d = ControllerInputDebouncer::new(50);
        assert!(d.should_process(100));
        assert!(!d.should_process(120));
        assert!(!d.should_process(140));
        assert_eq!(d.events_dropped(), 2);
        assert!(d.should_process(160));
        assert_eq!(d.events_dropped(), 2);
    }

    #[test]
    fn debouncer_reset() {
        let mut d = ControllerInputDebouncer::new(50);
        d.should_process(100);
        d.should_process(110);
        d.reset();
        assert_eq!(d.last_processed(), None);
        assert_eq!(d.events_dropped(), 0);
        assert!(d.should_process(10));
    }

    #[test]
    fn edbuf_ringbuf_push_get() {
        let mut rb = EdBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn edbuf_ringbuf_overflow() {
        let mut rb = EdBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn edbuf_ringbuf_clear() {
        let mut rb = EdBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn edbuf_ringbuf_newest_oldest() {
        let mut rb = EdBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn edbuf_ringbuf_to_vec() {
        let mut rb = EdBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn edbuf_ringbuf_is_full() {
        let mut rb = EdBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn edc_lru_insert_get() {
        let mut c = EdCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn edc_lru_eviction() {
        let mut c = EdCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn edc_lru_hit_ratio() {
        let mut c = EdCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn edc_lru_clear() {
        let mut c = EdCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn edc_lru_remove() {
        let mut c = EdCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn edc_lru_peek() {
        let mut c = EdCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    #[test]
    fn editor_controller_config_new() {
        let cfg = EditorControllerConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn editor_controller_config_set_get() {
        let mut cfg = EditorControllerConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn editor_controller_config_remove() {
        let mut cfg = EditorControllerConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn editor_controller_config_keys_sorted() {
        let mut cfg = EditorControllerConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn editor_controller_config_bump_version() {
        let mut cfg = EditorControllerConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn editor_controller_config_clear() {
        let mut cfg = EditorControllerConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn editor_controller_config_merge() {
        let mut cfg1 = EditorControllerConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = EditorControllerConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn editor_controller_config_disable() {
        let mut cfg = EditorControllerConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn editor_controller_rate_tracker_empty() {
        let rt = EditorControllerRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn editor_controller_rate_tracker_record() {
        let mut rt = EditorControllerRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn editor_controller_rate_tracker_prune() {
        let mut rt = EditorControllerRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn editor_controller_validator_valid() {
        let v = EditorControllerValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn editor_controller_validator_errors() {
        let mut v = EditorControllerValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn editor_controller_validator_clear() {
        let mut v = EditorControllerValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn editor_controller_validator_merge() {
        let mut v1 = EditorControllerValidator::new();
        v1.add_error("e1");
        let mut v2 = EditorControllerValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn editor_controller_rate_tracker_clear() {
        let mut rt = EditorControllerRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yh_metrics_empty() {
        let m = YhMetrics::new("editor_ctrl");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yh_metrics_record_and_mean() {
        let mut m = YhMetrics::new("editor_ctrl");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yh_metrics_min_max() {
        let mut m = YhMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yh_metrics_variance_and_std() {
        let mut m = YhMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yh_metrics_percentile() {
        let mut m = YhMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yh_metrics_merge() {
        let mut a = YhMetrics::new("a");
        a.record(1.0);
        let mut b = YhMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yh_metrics_reset() {
        let mut m = YhMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yh_rate_window_empty() {
        let rw = YhRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yh_rate_window_tick_and_rate() {
        let mut rw = YhRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yh_lru_cache_basic() {
        let mut c = YhLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yh_lru_cache_contains_and_keys() {
        let mut c = YhLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yh_lru_cache_remove() {
        let mut c = YhLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yh_metrics_sum() {
        let mut m = YhMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yh_metrics_label() {
        let m = YhMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yh_lru_cache_clear() {
        let mut c = YhLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for editor_controller
    #[test]
    fn xa_editor_controller_ring_new() {
        let rb = super::XaEditorControllerRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_editor_controller_ring_push_len() {
        let mut rb = super::XaEditorControllerRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_editor_controller_ring_wrap() {
        let mut rb = super::XaEditorControllerRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_editor_controller_ring_mean_empty() {
        let rb = super::XaEditorControllerRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_editor_controller_ring_mean_values() {
        let mut rb = super::XaEditorControllerRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_editor_controller_ring_min_max() {
        let mut rb = super::XaEditorControllerRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_editor_controller_ring_iter() {
        let mut rb = super::XaEditorControllerRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_editor_controller_counter_new() {
        let c = super::XaEditorControllerCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_controller_counter_inc() {
        let mut c = super::XaEditorControllerCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_editor_controller_counter_inc_by() {
        let mut c = super::XaEditorControllerCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_editor_controller_counter_reset() {
        let mut c = super::XaEditorControllerCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_editor_controller_counter_clear() {
        let mut c = super::XaEditorControllerCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_controller_counter_default() {
        let c = super::XaEditorControllerCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 35 ----

    #[test]
    fn xc_35_pool_new_empty() {
        let pool: super::Xc35Pool<i32> = super::Xc35Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_35_pool_release_acquire() {
        let mut pool = super::Xc35Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_35_pool_acquire_empty() {
        let mut pool: super::Xc35Pool<i32> = super::Xc35Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_35_pool_full() {
        let mut pool = super::Xc35Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_35_pool_drain() {
        let mut pool = super::Xc35Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_35_pool_stats() {
        let mut pool = super::Xc35Pool::new(8);
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
    fn xc_35_pool_clear() {
        let mut pool = super::Xc35Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_35_pool_shrink() {
        let mut pool = super::Xc35Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_35_pool_default() {
        let pool: super::Xc35Pool<String> = super::Xc35Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_35_pool_extend() {
        let mut pool = super::Xc35Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_35_pool_retain() {
        let mut pool = super::Xc35Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_35_scheduler_round_robin() {
        let mut sched = super::Xc35Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_35_scheduler_empty() {
        let mut sched = super::Xc35Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_35_scheduler_reset() {
        let mut sched = super::Xc35Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_35_scheduler_add_remove() {
        let mut sched = super::Xc35Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_35_scheduler_targets() {
        let sched = super::Xc35Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_35_hash_empty() {
        assert_eq!(super::xc_35_hash(b""), 5381);
    }

    #[test]
    fn xc_35_hash_data() {
        let h = super::xc_35_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_35_hash(b"hello"), h);
    }

    #[test]
    fn xc_35_reverse_str() {
        assert_eq!(super::xc_35_reverse("abc"), "cba");
        assert_eq!(super::xc_35_reverse(""), "");
    }


    // --- xd_85 deepening tests ---

    #[test]
    fn xd_85_sm_initial_state() {
        let sm = Xd85StateMachine::new();
        assert_eq!(sm.current_state(), Xd85State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_85_sm_valid_idle_to_running() {
        let mut sm = Xd85StateMachine::new();
        assert!(sm.transition(Xd85State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd85State::Running);
    }

    #[test]
    fn xd_85_sm_valid_running_to_paused() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        assert!(sm.transition(Xd85State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd85State::Paused);
    }

    #[test]
    fn xd_85_sm_valid_running_to_done() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        assert!(sm.transition(Xd85State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd85State::Done);
    }

    #[test]
    fn xd_85_sm_valid_paused_to_running() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        sm.transition(Xd85State::Paused).unwrap();
        assert!(sm.transition(Xd85State::Running).is_ok());
    }

    #[test]
    fn xd_85_sm_valid_done_to_idle() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        sm.transition(Xd85State::Done).unwrap();
        assert!(sm.transition(Xd85State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd85State::Idle);
    }

    #[test]
    fn xd_85_sm_invalid_idle_to_done() {
        let mut sm = Xd85StateMachine::new();
        assert!(sm.transition(Xd85State::Done).is_err());
    }

    #[test]
    fn xd_85_sm_invalid_idle_to_paused() {
        let mut sm = Xd85StateMachine::new();
        assert!(sm.transition(Xd85State::Paused).is_err());
    }

    #[test]
    fn xd_85_sm_history_tracking() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        sm.transition(Xd85State::Paused).unwrap();
        sm.transition(Xd85State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd85State::Idle);
        assert_eq!(sm.history()[0].to, Xd85State::Running);
        assert_eq!(sm.history()[1].from, Xd85State::Running);
        assert_eq!(sm.history()[2].to, Xd85State::Done);
    }

    #[test]
    fn xd_85_sm_serialize_deserialize() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd85StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd85State::Running));
    }

    #[test]
    fn xd_85_sm_deserialize_invalid() {
        assert_eq!(Xd85StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_85_sm_reset() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd85State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_85_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd85EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd85Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_85_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd85EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd85Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd85Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_85_bus_unsubscribe() {
        let mut bus = Xd85EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_85_event_kind_and_payload() {
        let e = Xd85Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd85Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_85_bus_clear_history() {
        let mut bus = Xd85EventBus::new();
        bus.publish(Xd85Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_85_sm_step_counter_increments() {
        let mut sm = Xd85StateMachine::new();
        sm.transition(Xd85State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd85State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #106 --

    #[test]
    fn xf106_trie_insert_search() {
        let mut t = Xf106Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf106_trie_starts_with() {
        let mut t = Xf106Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf106_trie_remove() {
        let mut t = Xf106Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf106_trie_word_count() {
        let mut t = Xf106Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf106_trie_longest_prefix() {
        let mut t = Xf106Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf106_trie_all_words() {
        let mut t = Xf106Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf106_trie_autocomplete() {
        let mut t = Xf106Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf106_trie_empty_search() {
        let t = Xf106Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf106_bloom_add_contains() {
        let mut bf = Xf106BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf106_bloom_probably_absent() {
        let bf = Xf106BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf106_bloom_false_positive_rate() {
        let mut bf = Xf106BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf106_bloom_clear() {
        let mut bf = Xf106BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf106_bloom_union() {
        let mut a = Xf106BloomFilter::xf_new(512, 2);
        let mut b = Xf106BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf106_bloom_intersection_estimate() {
        let mut a = Xf106BloomFilter::xf_new(512, 2);
        let mut b = Xf106BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf106_bloom_union_size_mismatch() {
        let a = Xf106BloomFilter::xf_new(256, 2);
        let b = Xf106BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh34_skip_insert_contains() {
        let mut sl = super::Xh34SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh34_skip_remove() {
        let mut sl = super::Xh34SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh34_skip_len() {
        let mut sl = super::Xh34SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh34_skip_range_query() {
        let mut sl = super::Xh34SkipList::xh_new(4);
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
    fn xh34_skip_floor_ceiling() {
        let mut sl = super::Xh34SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh34_skip_rank() {
        let mut sl = super::Xh34SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh34_skip_empty() {
        let sl = super::Xh34SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh34_skip_duplicates() {
        let mut sl = super::Xh34SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh34_bitset_set_test() {
        let mut bs = super::Xh34BitSet::xh_new(256);
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
    fn xh34_bitset_clear_count() {
        let mut bs = super::Xh34BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh34_bitset_and_or_xor() {
        let mut a = super::Xh34BitSet::xh_new(128);
        let mut b = super::Xh34BitSet::xh_new(128);
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
    fn xh34_bitset_iter_ones() {
        let mut bs = super::Xh34BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh34_bitset_first_last() {
        let mut bs = super::Xh34BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh34_bitset_empty() {
        let bs = super::Xh34BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi34_deque_push_pop_back() {
        let mut dq = super::Xi34Deque::xi_new(4);
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
    fn xi34_deque_push_pop_front() {
        let mut dq = super::Xi34Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi34_deque_mixed_ops() {
        let mut dq = super::Xi34Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi34_deque_get_and_split() {
        let mut dq = super::Xi34Deque::xi_new(8);
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
    fn xi34_deque_rotate_left() {
        let mut dq = super::Xi34Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi34_deque_rotate_right() {
        let mut dq = super::Xi34Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi34_deque_grow() {
        let mut dq = super::Xi34Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi34_deque_empty() {
        let dq = super::Xi34Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi34_interval_tree_insert_query() {
        let mut tree = super::Xi34IntervalTree::xi_new();
        tree.xi_insert(super::Xi34Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi34Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi34Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi34_interval_tree_overlap() {
        let mut tree = super::Xi34IntervalTree::xi_new();
        tree.xi_insert(super::Xi34Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi34Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi34Interval::xi_new(12, 20));
        let q = super::Xi34Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi34_interval_tree_remove() {
        let mut tree = super::Xi34IntervalTree::xi_new();
        tree.xi_insert(super::Xi34Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi34Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi34_interval_tree_gaps() {
        let mut tree = super::Xi34IntervalTree::xi_new();
        tree.xi_insert(super::Xi34Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi34Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi34Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi34Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi34Interval::xi_new(8, 10));
    }

    #[test]
    fn xi34_interval_tree_merge() {
        let mut tree = super::Xi34IntervalTree::xi_new();
        tree.xi_insert(super::Xi34Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi34Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi34Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi34Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi34Interval::xi_new(10, 15));
    }

    #[test]
    fn xi34_interval_tree_all() {
        let mut tree = super::Xi34IntervalTree::xi_new();
        tree.xi_insert(super::Xi34Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi34Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi34_interval_tree_empty() {
        let tree = super::Xi34IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi34_interval_tree_contains_point() {
        let iv = super::Xi34Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}