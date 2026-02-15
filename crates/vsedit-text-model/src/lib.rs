//! Core text buffer for vsedit, backed by [`ropey::Rope`].
//!
//! This crate is the Rust equivalent of VS Code's `vs/editor/common/model/textModel.ts`.
//! It provides the [`TextModel`] struct which owns the document text, supports efficient
//! editing, position/offset conversion, undo/redo, search, and fires change events.

use std::cell::UnsafeCell;

use regex::Regex;
use ropey::Rope;
use vsedit_editor_types::{ITextModel, Position, Range};
use vsedit_events::Emitter;
use vsedit_undoredo::UndoRedoStack;

// ---------------------------------------------------------------------------
// ModelContentChangedEvent
// ---------------------------------------------------------------------------

/// Describes a content change applied to a [`TextModel`].
#[derive(Debug, Clone)]
pub struct ModelContentChangedEvent {
    /// The range that was replaced (in the old document).
    pub range: Range,
    /// The text that was inserted.
    pub text: String,
    /// The length of the text that was replaced.
    pub range_length: usize,
}

// ---------------------------------------------------------------------------
// EditOperation (for undo/redo)
// ---------------------------------------------------------------------------

/// A single reversible edit operation.
#[derive(Debug, Clone)]
struct EditOperation {
    /// The range in the *new* document that was affected.
    range_after: Range,
    /// The text that was inserted (to reverse, delete this text).
    text_inserted: String,
    /// The text that was replaced (to reverse, re-insert this text).
    text_replaced: String,
    /// The range in the *old* document that was replaced.
    range_before: Range,
}

// ---------------------------------------------------------------------------
// TextModel
// ---------------------------------------------------------------------------

/// The core text document.
///
/// Uses a [`Rope`] internally for efficient large-document editing, and
/// implements [`ITextModel`] for integration with editor components.
pub struct TextModel {
    rope: Rope,
    /// Cached line content for `get_line_content` (returns `&str`).
    /// Uses `UnsafeCell` so `get_line_content` can return `&str` from `&self`.
    line_cache: UnsafeCell<(u32, String)>,
    on_did_change_content_emitter: Emitter<ModelContentChangedEvent>,
    undo_stack: UndoRedoStack<EditOperation>,
}

impl TextModel {
    /// Create a new `TextModel` from a string.
    pub fn new(content: &str) -> Self {
        Self {
            rope: Rope::from_str(content),
            line_cache: UnsafeCell::new((0, String::new())),
            on_did_change_content_emitter: Emitter::new(),
            undo_stack: UndoRedoStack::new(),
        }
    }

    /// Create an empty `TextModel`.
    pub fn empty() -> Self {
        Self::new("")
    }

    // -- Event accessor -----------------------------------------------------

    /// Returns the event that fires after content changes.
    pub fn on_did_change_content(&self) -> vsedit_events::Event<ModelContentChangedEvent> {
        self.on_did_change_content_emitter.event()
    }

    // -- Full content access -------------------------------------------------

    /// Returns the full content of the model as a `String`.
    pub fn get_value(&self) -> String {
        self.rope.to_string()
    }

    /// Returns the text within the given range.
    pub fn get_value_in_range(&self, range: Range) -> String {
        let range = self.validate_range(range);
        let start = self.position_to_offset(range.start);
        let end = self.position_to_offset(range.end);
        let start_char = self.rope.byte_to_char(start);
        let end_char = self.rope.byte_to_char(end);
        self.rope.slice(start_char..end_char).to_string()
    }

    // -- Position / offset conversion ----------------------------------------

    /// Convert a byte offset to a 1-based `Position`.
    pub fn offset_to_position(&self, offset: usize) -> Position {
        let offset = offset.min(self.rope.len_bytes());
        let char_idx = self.rope.byte_to_char(offset);
        let line_idx = self.rope.char_to_line(char_idx);
        let line_start_char = self.rope.line_to_char(line_idx);
        let col_chars = char_idx - line_start_char;
        // Convert char offset within line back to byte offset within line
        let line_slice = self.rope.line(line_idx);
        let col_bytes = if col_chars == 0 {
            0
        } else {
            let mut bytes = 0;
            for (i, ch) in line_slice.chars().enumerate() {
                if i >= col_chars {
                    break;
                }
                bytes += ch.len_utf8();
            }
            bytes
        };
        Position::new((line_idx + 1) as u32, (col_bytes + 1) as u32)
    }

    /// Convert a 1-based `Position` to a byte offset.
    pub fn position_to_offset(&self, position: Position) -> usize {
        let pos = self.validate_position(position);
        let line_idx = (pos.line - 1) as usize;
        let line_start_char = self.rope.line_to_char(line_idx);
        let line_slice = self.rope.line(line_idx);
        // Column is 1-based byte offset; convert to char offset
        let col_bytes = (pos.column - 1) as usize;
        let mut chars_count = 0;
        let mut bytes_count = 0;
        for ch in line_slice.chars() {
            if bytes_count >= col_bytes {
                break;
            }
            bytes_count += ch.len_utf8();
            chars_count += 1;
        }
        let char_idx = line_start_char + chars_count;
        self.rope.char_to_byte(char_idx)
    }

    /// Clamp a position to valid document bounds.
    pub fn validate_position(&self, position: Position) -> Position {
        let line_count = self.get_line_count();
        if line_count == 0 {
            return Position::new(1, 1);
        }
        let line = position.line.max(1).min(line_count);
        let max_col = self.get_line_max_column(line);
        let column = position.column.max(1).min(max_col);
        Position::new(line, column)
    }

    /// Clamp a range to valid document bounds.
    pub fn validate_range(&self, range: Range) -> Range {
        let start = self.validate_position(range.start);
        let end = self.validate_position(range.end);
        Range::from_positions(start, end)
    }

    // -- Edit operations -----------------------------------------------------

    /// Replace the text in `range` with `text`.
    pub fn apply_edit(&mut self, range: Range, text: &str) {
        let range = self.validate_range(range);
        let replaced_text = self.get_value_in_range(range);
        let range_length = replaced_text.len();

        let start_offset = self.position_to_offset(range.start);
        let end_offset = self.position_to_offset(range.end);
        let start_char = self.rope.byte_to_char(start_offset);
        let end_char = self.rope.byte_to_char(end_offset);

        // Perform the edit on the rope
        self.rope.remove(start_char..end_char);
        if !text.is_empty() {
            self.rope.insert(start_char, text);
        }

        // Invalidate line cache
        // SAFETY: No outstanding references to the cache exist during mutation.
        unsafe { (*self.line_cache.get()).0 = 0; }

        // Compute the range after edit for undo
        let end_after = self.offset_to_position(start_offset + text.len());
        let range_after = Range::from_positions(range.start, end_after);

        let edit_op = EditOperation {
            range_after,
            text_inserted: text.to_string(),
            text_replaced: replaced_text,
            range_before: range,
        };
        self.undo_stack.push(edit_op);

        let event = ModelContentChangedEvent {
            range,
            text: text.to_string(),
            range_length,
        };
        self.on_did_change_content_emitter.fire(&event);
    }

    /// Insert text at a position.
    pub fn insert(&mut self, position: Position, text: &str) {
        let pos = self.validate_position(position);
        self.apply_edit(Range::from_positions(pos, pos), text);
    }

    /// Delete text in a range.
    pub fn delete(&mut self, range: Range) {
        self.apply_edit(range, "");
    }

    /// Apply a batch of edits and push to the undo stack.
    ///
    /// Edits are applied in reverse order (bottom-to-top) so that earlier
    /// ranges remain valid.
    pub fn push_edit_operations(&mut self, edits: &[(Range, String)]) {
        let mut sorted: Vec<(Range, String)> = edits.to_vec();
        sorted.sort_by(|a, b| b.0.start.cmp(&a.0.start));
        for (range, text) in sorted {
            self.apply_edit(range, &text);
        }
    }

    // -- Undo / Redo ---------------------------------------------------------

    /// Undo the last edit operation.
    pub fn undo(&mut self) -> bool {
        // We need to pop from our stack without going through apply_edit
        // (which would push another undo entry).
        let op = match self.undo_stack.undo() {
            Some(op) => op,
            None => return false,
        };

        // Reverse the edit: replace the inserted text with the original text
        let start_offset = self.position_to_offset(op.range_after.start);
        let end_offset = self.position_to_offset(op.range_after.end);
        let start_char = self.rope.byte_to_char(start_offset);
        let end_char = self.rope.byte_to_char(end_offset);

        self.rope.remove(start_char..end_char);
        if !op.text_replaced.is_empty() {
            self.rope.insert(start_char, &op.text_replaced);
        }
        unsafe { (*self.line_cache.get()).0 = 0; }

        let event = ModelContentChangedEvent {
            range: op.range_after,
            text: op.text_replaced.clone(),
            range_length: op.text_inserted.len(),
        };
        self.on_did_change_content_emitter.fire(&event);
        true
    }

    /// Redo the last undone edit operation.
    pub fn redo(&mut self) -> bool {
        let op = match self.undo_stack.redo() {
            Some(op) => op,
            None => return false,
        };

        // Re-apply the edit
        let start_offset = self.position_to_offset(op.range_before.start);
        let end_offset = self.position_to_offset(op.range_before.end);
        let start_char = self.rope.byte_to_char(start_offset);
        let end_char = self.rope.byte_to_char(end_offset);

        self.rope.remove(start_char..end_char);
        if !op.text_inserted.is_empty() {
            self.rope.insert(start_char, &op.text_inserted);
        }
        unsafe { (*self.line_cache.get()).0 = 0; }

        let event = ModelContentChangedEvent {
            range: op.range_before,
            text: op.text_inserted.clone(),
            range_length: op.text_replaced.len(),
        };
        self.on_did_change_content_emitter.fire(&event);
        true
    }

    // -- Search --------------------------------------------------------------

    /// Find all matches of a search string in the document.
    ///
    /// When `is_regex` is `true`, `search_string` is treated as a regular
    /// expression pattern.
    pub fn find_matches(
        &self,
        search_string: &str,
        is_regex: bool,
        case_sensitive: bool,
    ) -> Vec<Range> {
        let content = self.get_value();
        let pattern = if is_regex {
            search_string.to_string()
        } else {
            regex::escape(search_string)
        };
        let re = if case_sensitive {
            Regex::new(&pattern)
        } else {
            Regex::new(&format!("(?i){}", pattern))
        };
        let re = match re {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();
        for mat in re.find_iter(&content) {
            let start_pos = self.offset_to_position(mat.start());
            let end_pos = self.offset_to_position(mat.end());
            results.push(Range::from_positions(start_pos, end_pos));
        }
        results
    }

    // -- Internal helpers ----------------------------------------------------

    /// Cache and return the line content (without trailing newline) for the
    /// given 1-based line number.
    fn cache_line(&self, line_number: u32) {
        // SAFETY: This is the only method that writes to the cache through
        // `&self`. It is never called concurrently (single-threaded access
        // enforced by the borrow checker on `&mut self` for mutations).
        let cache = unsafe { &mut *self.line_cache.get() };
        if cache.0 == line_number {
            return;
        }
        let line_idx = (line_number - 1) as usize;
        let line_slice = self.rope.line(line_idx);
        let mut s = line_slice.to_string();
        // Strip trailing newline characters
        if s.ends_with("\r\n") {
            s.truncate(s.len() - 2);
        } else if s.ends_with('\n') || s.ends_with('\r') {
            s.truncate(s.len() - 1);
        }
        *cache = (line_number, s);
    }
}

// ---------------------------------------------------------------------------
// ITextModel implementation
// ---------------------------------------------------------------------------

impl ITextModel for TextModel {
    fn get_line_count(&self) -> u32 {
        self.rope.len_lines() as u32
    }

    fn get_line_content(&self, line_number: u32) -> &str {
        self.cache_line(line_number);
        // SAFETY: The cache is only mutated by `cache_line` (which we just
        // called) or by `&mut self` methods. Since we hold `&self`, no
        // `&mut self` method can run, so the pointer is stable.
        unsafe { (*self.line_cache.get()).1.as_str() }
    }

    fn get_line_length(&self, line_number: u32) -> u32 {
        self.cache_line(line_number);
        unsafe { (&(*self.line_cache.get()).1).len() as u32 }
    }

    fn get_line_max_column(&self, line_number: u32) -> u32 {
        self.get_line_length(line_number) + 1
    }

    fn get_value_length(&self) -> usize {
        self.rope.len_bytes()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // -- Construction -------------------------------------------------------

    #[test]
    fn create_from_string() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.get_value(), "hello\nworld");
        assert_eq!(model.get_line_count(), 2);
    }

    #[test]
    fn create_empty() {
        let model = TextModel::empty();
        assert_eq!(model.get_value(), "");
        assert_eq!(model.get_line_count(), 1);
    }

    // -- ITextModel trait ----------------------------------------------------

    #[test]
    fn line_count_single() {
        let model = TextModel::new("abc");
        assert_eq!(model.get_line_count(), 1);
    }

    #[test]
    fn line_count_multi() {
        let model = TextModel::new("a\nb\nc");
        assert_eq!(model.get_line_count(), 3);
    }

    #[test]
    fn line_count_trailing_newline() {
        let model = TextModel::new("a\nb\n");
        assert_eq!(model.get_line_count(), 3);
    }

    #[test]
    fn get_line_content_basic() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.get_line_content(1), "hello");
        assert_eq!(model.get_line_content(2), "world");
    }

    #[test]
    fn get_line_length_basic() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.get_line_length(1), 5);
        assert_eq!(model.get_line_length(2), 5);
    }

    #[test]
    fn get_line_max_column() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.get_line_max_column(1), 6);
    }

    #[test]
    fn get_value_length() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.get_value_length(), 11);
    }

    // -- Position / offset conversion ----------------------------------------

    #[test]
    fn offset_to_position_basic() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.offset_to_position(0), Position::new(1, 1));
        assert_eq!(model.offset_to_position(5), Position::new(1, 6));
        assert_eq!(model.offset_to_position(6), Position::new(2, 1));
        assert_eq!(model.offset_to_position(11), Position::new(2, 6));
    }

    #[test]
    fn position_to_offset_basic() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.position_to_offset(Position::new(1, 1)), 0);
        assert_eq!(model.position_to_offset(Position::new(1, 6)), 5);
        assert_eq!(model.position_to_offset(Position::new(2, 1)), 6);
        assert_eq!(model.position_to_offset(Position::new(2, 6)), 11);
    }

    #[test]
    fn offset_position_roundtrip() {
        let model = TextModel::new("hello\nworld\nfoo");
        for offset in 0..model.get_value_length() {
            let pos = model.offset_to_position(offset);
            let back = model.position_to_offset(pos);
            assert_eq!(back, offset, "roundtrip failed for offset {offset}");
        }
    }

    #[test]
    fn validate_position_clamps() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(
            model.validate_position(Position::new(0, 0)),
            Position::new(1, 1)
        );
        assert_eq!(
            model.validate_position(Position::new(100, 100)),
            Position::new(2, 6)
        );
        assert_eq!(
            model.validate_position(Position::new(1, 100)),
            Position::new(1, 6)
        );
    }

    #[test]
    fn validate_range_clamps() {
        let model = TextModel::new("hello\nworld");
        let r = model.validate_range(Range::new(0, 0, 100, 100));
        assert_eq!(r.start, Position::new(1, 1));
        assert_eq!(r.end, Position::new(2, 6));
    }

    // -- Edit operations -----------------------------------------------------

    #[test]
    fn insert_at_beginning() {
        let mut model = TextModel::new("world");
        model.insert(Position::new(1, 1), "hello ");
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn insert_at_end() {
        let mut model = TextModel::new("hello");
        model.insert(Position::new(1, 6), " world");
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn insert_newline() {
        let mut model = TextModel::new("helloworld");
        model.insert(Position::new(1, 6), "\n");
        assert_eq!(model.get_value(), "hello\nworld");
        assert_eq!(model.get_line_count(), 2);
        assert_eq!(model.get_line_content(1), "hello");
        assert_eq!(model.get_line_content(2), "world");
    }

    #[test]
    fn delete_range() {
        let mut model = TextModel::new("hello world");
        model.delete(Range::new(1, 6, 1, 12));
        assert_eq!(model.get_value(), "hello");
    }

    #[test]
    fn delete_across_lines() {
        let mut model = TextModel::new("hello\nworld");
        model.delete(Range::new(1, 6, 2, 1));
        assert_eq!(model.get_value(), "helloworld");
        assert_eq!(model.get_line_count(), 1);
    }

    #[test]
    fn apply_edit_replace() {
        let mut model = TextModel::new("hello world");
        model.apply_edit(Range::new(1, 7, 1, 12), "rust");
        assert_eq!(model.get_value(), "hello rust");
    }

    #[test]
    fn get_value_in_range_basic() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(
            model.get_value_in_range(Range::new(1, 1, 1, 6)),
            "hello"
        );
        assert_eq!(
            model.get_value_in_range(Range::new(2, 1, 2, 6)),
            "world"
        );
        assert_eq!(
            model.get_value_in_range(Range::new(1, 1, 2, 6)),
            "hello\nworld"
        );
    }

    #[test]
    fn multi_line_insert() {
        let mut model = TextModel::new("ac");
        model.insert(Position::new(1, 2), "b\nd\ne");
        assert_eq!(model.get_value(), "ab\nd\nec");
        assert_eq!(model.get_line_count(), 3);
    }

    #[test]
    fn push_edit_operations_batch() {
        let mut model = TextModel::new("aabbcc");
        model.push_edit_operations(&[
            (Range::new(1, 1, 1, 3), "AA".to_string()),
            (Range::new(1, 5, 1, 7), "CC".to_string()),
        ]);
        assert_eq!(model.get_value(), "AAbbCC");
    }

    // -- Undo / Redo ---------------------------------------------------------

    #[test]
    fn undo_single_edit() {
        let mut model = TextModel::new("hello");
        model.insert(Position::new(1, 6), " world");
        assert_eq!(model.get_value(), "hello world");
        assert!(model.undo());
        assert_eq!(model.get_value(), "hello");
    }

    #[test]
    fn redo_single_edit() {
        let mut model = TextModel::new("hello");
        model.insert(Position::new(1, 6), " world");
        model.undo();
        assert!(model.redo());
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn undo_delete() {
        let mut model = TextModel::new("hello world");
        model.delete(Range::new(1, 6, 1, 12));
        assert_eq!(model.get_value(), "hello");
        model.undo();
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn undo_replace() {
        let mut model = TextModel::new("hello world");
        model.apply_edit(Range::new(1, 7, 1, 12), "rust");
        assert_eq!(model.get_value(), "hello rust");
        model.undo();
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut model = TextModel::new("abc");
        model.insert(Position::new(1, 4), "def");
        model.insert(Position::new(1, 7), "ghi");
        assert_eq!(model.get_value(), "abcdefghi");
        model.undo();
        assert_eq!(model.get_value(), "abcdef");
        model.undo();
        assert_eq!(model.get_value(), "abc");
        model.redo();
        assert_eq!(model.get_value(), "abcdef");
        model.redo();
        assert_eq!(model.get_value(), "abcdefghi");
    }

    #[test]
    fn undo_empty_returns_false() {
        let mut model = TextModel::new("hello");
        assert!(!model.undo());
    }

    #[test]
    fn redo_empty_returns_false() {
        let mut model = TextModel::new("hello");
        assert!(!model.redo());
    }

    // -- Events --------------------------------------------------------------

    #[test]
    fn on_did_change_content_fires() {
        let mut model = TextModel::new("hello");
        let events: Arc<Mutex<Vec<ModelContentChangedEvent>>> =
            Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let _handle = model.on_did_change_content().on(move |e| {
            events_clone.lock().unwrap().push(e.clone());
        });

        model.insert(Position::new(1, 6), " world");

        let evts = events.lock().unwrap();
        assert_eq!(evts.len(), 1);
        assert_eq!(evts[0].text, " world");
    }

    // -- Search --------------------------------------------------------------

    #[test]
    fn find_matches_literal() {
        let model = TextModel::new("hello world hello");
        let matches = model.find_matches("hello", false, true);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], Range::new(1, 1, 1, 6));
        assert_eq!(matches[1], Range::new(1, 13, 1, 18));
    }

    #[test]
    fn find_matches_case_insensitive() {
        let model = TextModel::new("Hello HELLO hello");
        let matches = model.find_matches("hello", false, false);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_matches_regex() {
        let model = TextModel::new("foo123 bar456");
        let matches = model.find_matches(r"\d+", true, true);
        assert_eq!(matches.len(), 2);
        assert_eq!(
            model.get_value_in_range(matches[0]),
            "123"
        );
        assert_eq!(
            model.get_value_in_range(matches[1]),
            "456"
        );
    }

    #[test]
    fn find_matches_no_match() {
        let model = TextModel::new("hello world");
        let matches = model.find_matches("xyz", false, true);
        assert!(matches.is_empty());
    }

    #[test]
    fn find_matches_multi_line() {
        let model = TextModel::new("hello\nworld\nhello");
        let matches = model.find_matches("hello", false, true);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], Range::new(1, 1, 1, 6));
        assert_eq!(matches[1], Range::new(3, 1, 3, 6));
    }

    #[test]
    fn find_matches_invalid_regex_returns_empty() {
        let model = TextModel::new("hello");
        let matches = model.find_matches("[invalid", true, true);
        assert!(matches.is_empty());
    }
}
