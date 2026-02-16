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
}
