//! Core text buffer for vsedit, backed by [`ropey::Rope`].
//!
//! This crate is the Rust equivalent of VS Code's `vs/editor/common/model/textModel.ts`.
//! It provides the [`TextModel`] struct which owns the document text, supports efficient
//! editing, position/offset conversion, undo/redo, search, and fires change events.
//!
//! Additional features matching VS Code's text buffer:
//! - Line ending detection/normalization (LF, CRLF)
//! - Encoding detection/conversion (UTF-8, UTF-8 BOM, UTF-16LE/BE, Latin1, etc.)
//! - Large file detection and truncated preview
//! - Grouped undo/redo with cursor state restoration
//! - Content change events with version tracking
//! - Immutable snapshots for async operations

use std::cell::UnsafeCell;
use std::path::Path;

use regex::Regex;
use ropey::Rope;
use vsedit_editor_types::{ITextModel, Position, Range};
use vsedit_events::Emitter;
use vsedit_undoredo::{CursorState, UndoRedoGroup, UndoRedoService, UndoRedoStack};

// ---------------------------------------------------------------------------
// LineEnding
// ---------------------------------------------------------------------------

/// Line ending style, matching VS Code's `EndOfLineSequence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style `\n`.
    LF,
    /// Windows-style `\r\n`.
    CRLF,
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::LF => "\n",
            LineEnding::CRLF => "\r\n",
        }
    }
}

/// Result of line ending detection — may be uniform or mixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedLineEnding {
    LF,
    CRLF,
    Mixed,
}

/// Detect the predominant line ending in `text`.
pub fn detect_line_ending(text: &str) -> DetectedLineEnding {
    let mut lf_count = 0u32;
    let mut crlf_count = 0u32;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'\r' && i + 1 < len && bytes[i + 1] == b'\n' {
            crlf_count += 1;
            i += 2;
        } else if bytes[i] == b'\n' {
            lf_count += 1;
            i += 1;
        } else {
            i += 1;
        }
    }
    if lf_count > 0 && crlf_count > 0 {
        DetectedLineEnding::Mixed
    } else if crlf_count > 0 {
        DetectedLineEnding::CRLF
    } else {
        DetectedLineEnding::LF
    }
}

/// Normalize all line endings in `text` to `target`.
pub fn normalize_line_endings(text: &str, target: LineEnding) -> String {
    // First normalize everything to LF, then convert to target.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    match target {
        LineEnding::LF => normalized,
        LineEnding::CRLF => normalized.replace('\n', "\r\n"),
    }
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

/// Text encoding, matching VS Code's supported encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    UTF8,
    UTF8BOM,
    UTF16LE,
    UTF16BE,
    Latin1,
    ShiftJIS,
    GBK,
}

/// Detect encoding from raw bytes by checking BOM markers, then falling back
/// to UTF-8.
pub fn detect_encoding(bytes: &[u8]) -> Encoding {
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return Encoding::UTF8BOM;
    }
    if bytes.len() >= 2 {
        if bytes[0] == 0xFF && bytes[1] == 0xFE {
            return Encoding::UTF16LE;
        }
        if bytes[0] == 0xFE && bytes[1] == 0xFF {
            return Encoding::UTF16BE;
        }
    }
    // Try UTF-8 validation
    if std::str::from_utf8(bytes).is_ok() {
        return Encoding::UTF8;
    }
    // Fallback to Latin1 for arbitrary byte sequences
    Encoding::Latin1
}

/// Decode bytes to a String using the given encoding.
pub fn decode_text(bytes: &[u8], encoding: Encoding) -> String {
    match encoding {
        Encoding::UTF8 => String::from_utf8_lossy(bytes).into_owned(),
        Encoding::UTF8BOM => {
            let start = if bytes.len() >= 3
                && bytes[0] == 0xEF
                && bytes[1] == 0xBB
                && bytes[2] == 0xBF
            {
                3
            } else {
                0
            };
            String::from_utf8_lossy(&bytes[start..]).into_owned()
        }
        Encoding::UTF16LE => {
            let start = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
                2
            } else {
                0
            };
            let u16s: Vec<u16> = bytes[start..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u16s)
        }
        Encoding::UTF16BE => {
            let start = if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                2
            } else {
                0
            };
            let u16s: Vec<u16> = bytes[start..]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u16s)
        }
        Encoding::Latin1 => bytes.iter().map(|&b| b as char).collect(),
        // ShiftJIS and GBK: lossy fallback — full codec support would
        // require the `encoding_rs` crate. For now treat as Latin1.
        Encoding::ShiftJIS | Encoding::GBK => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Encode a String to bytes using the given encoding.
pub fn encode_text(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::UTF8 => text.as_bytes().to_vec(),
        Encoding::UTF8BOM => {
            let mut out = vec![0xEF, 0xBB, 0xBF];
            out.extend_from_slice(text.as_bytes());
            out
        }
        Encoding::UTF16LE => {
            let mut out = vec![0xFF, 0xFE]; // BOM
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        Encoding::UTF16BE => {
            let mut out = vec![0xFE, 0xFF]; // BOM
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
        Encoding::Latin1 | Encoding::ShiftJIS | Encoding::GBK => {
            text.chars().map(|c| c as u8).collect()
        }
    }
}

// ---------------------------------------------------------------------------
// FileEncoding — lightweight encoding detection with self-contained methods
// ---------------------------------------------------------------------------

/// Detected file encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Latin1,
}

impl FileEncoding {
    /// Detect encoding from raw bytes by examining BOM and byte patterns.
    pub fn detect(data: &[u8]) -> Self {
        // Check for BOM
        if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return Self::Utf8Bom;
        }
        if data.starts_with(&[0xFF, 0xFE]) {
            return Self::Utf16Le;
        }
        if data.starts_with(&[0xFE, 0xFF]) {
            return Self::Utf16Be;
        }
        // Try UTF-8
        if std::str::from_utf8(data).is_ok() {
            return Self::Utf8;
        }
        // Fallback to Latin1 (always valid for any byte sequence)
        Self::Latin1
    }

    /// Decode bytes to string using detected encoding.
    pub fn decode(&self, data: &[u8]) -> String {
        match self {
            Self::Utf8 => String::from_utf8_lossy(data).into_owned(),
            Self::Utf8Bom => String::from_utf8_lossy(&data[3..]).into_owned(),
            Self::Utf16Le => {
                let chars: Vec<u16> = data[2..]
                    .chunks(2)
                    .map(|c| u16::from_le_bytes([c[0], c.get(1).copied().unwrap_or(0)]))
                    .collect();
                String::from_utf16_lossy(&chars)
            }
            Self::Utf16Be => {
                let chars: Vec<u16> = data[2..]
                    .chunks(2)
                    .map(|c| u16::from_be_bytes([c[0], c.get(1).copied().unwrap_or(0)]))
                    .collect();
                String::from_utf16_lossy(&chars)
            }
            Self::Latin1 => data.iter().map(|&b| b as char).collect(),
        }
    }

    /// Encode string back to bytes using this encoding.
    pub fn encode(&self, text: &str) -> Vec<u8> {
        match self {
            Self::Utf8 => text.as_bytes().to_vec(),
            Self::Utf8Bom => {
                let mut buf = vec![0xEF, 0xBB, 0xBF];
                buf.extend_from_slice(text.as_bytes());
                buf
            }
            Self::Utf16Le => {
                let mut buf = vec![0xFF, 0xFE];
                for c in text.encode_utf16() {
                    buf.extend_from_slice(&c.to_le_bytes());
                }
                buf
            }
            Self::Utf16Be => {
                let mut buf = vec![0xFE, 0xFF];
                for c in text.encode_utf16() {
                    buf.extend_from_slice(&c.to_be_bytes());
                }
                buf
            }
            Self::Latin1 => text.chars().map(|c| c as u8).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Large file support
// ---------------------------------------------------------------------------

/// 50 MB threshold matching VS Code's large file limit.
const LARGE_FILE_THRESHOLD: u64 = 50 * 1024 * 1024;

/// Returns true if the file at `path` exceeds 50 MB.
pub fn is_large_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > LARGE_FILE_THRESHOLD)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// ContentChange & ModelContentChangedEvent
// ---------------------------------------------------------------------------

/// A single content change within an edit, matching VS Code's
/// `IModelContentChange`.
#[derive(Debug, Clone)]
pub struct ContentChange {
    /// The range that was replaced (in the old document).
    pub range: Range,
    /// The text that was inserted.
    pub text: String,
    /// Byte offset of the range start in the old document.
    pub range_offset: usize,
    /// Byte length of the range in the old document.
    pub range_length: usize,
}

/// Event fired after content changes, matching VS Code's
/// `IModelContentChangedEvent`.
#[derive(Debug, Clone)]
pub struct ModelContentChangedEvent {
    /// Individual content changes.
    pub changes: Vec<ContentChange>,
    /// The version id after this change.
    pub version_id: u64,
    /// True if this change was produced by an undo operation.
    pub is_undo: bool,
    /// True if this change was produced by a redo operation.
    pub is_redo: bool,
}

// ---------------------------------------------------------------------------
// EditOperation
// ---------------------------------------------------------------------------

/// A single reversible edit operation, public for grouped undo/redo.
#[derive(Debug, Clone)]
pub struct EditOperation {
    /// The range in the *new* document that was affected.
    pub range_after: Range,
    /// The text that was inserted.
    pub text_inserted: String,
    /// The text that was replaced.
    pub text_replaced: String,
    /// The range in the *old* document that was replaced.
    pub range_before: Range,
    /// Whether to force move markers past the edit.
    pub force_move_markers: bool,
}

// ---------------------------------------------------------------------------
// ModelSnapshot
// ---------------------------------------------------------------------------

/// An immutable snapshot of the model for async operations, matching VS Code's
/// `ITextSnapshot`.
#[derive(Debug, Clone)]
pub struct ModelSnapshot {
    pub text: String,
    pub version_id: u64,
    pub line_ending: LineEnding,
    pub encoding: Encoding,
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
    line_cache: UnsafeCell<(u32, String)>,
    on_did_change_content_emitter: Emitter<ModelContentChangedEvent>,
    /// Legacy flat undo stack (kept for backward compatibility).
    undo_stack: UndoRedoStack<EditOperation>,
    /// Grouped undo/redo service with cursor state.
    undo_service: UndoRedoService<EditOperation>,
    /// Monotonically increasing version id.
    version_id: u64,
    /// Alternative version id that accounts for undo/redo returning to a
    /// previous state.
    alternative_version_id: u64,
    /// Detected/configured line ending.
    line_ending: LineEnding,
    /// Detected/configured encoding.
    encoding: Encoding,
    /// Detected file encoding (lightweight variant).
    pub file_encoding: FileEncoding,
}

impl TextModel {
    /// Create a new `TextModel` from a string.
    pub fn new(content: &str) -> Self {
        let eol = match detect_line_ending(content) {
            DetectedLineEnding::CRLF => LineEnding::CRLF,
            _ => LineEnding::LF,
        };
        Self {
            rope: Rope::from_str(content),
            line_cache: UnsafeCell::new((0, String::new())),
            on_did_change_content_emitter: Emitter::new(),
            undo_stack: UndoRedoStack::new(),
            undo_service: UndoRedoService::new(),
            version_id: 1,
            alternative_version_id: 1,
            line_ending: eol,
            encoding: Encoding::UTF8,
            file_encoding: FileEncoding::Utf8,
        }
    }

    /// Create an empty `TextModel`.
    pub fn empty() -> Self {
        Self::new("")
    }

    /// Create a `TextModel` from raw bytes, auto-detecting encoding.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let enc = detect_encoding(bytes);
        let file_enc = FileEncoding::detect(bytes);
        let text = decode_text(bytes, enc);
        let mut model = Self::new(&text);
        model.encoding = enc;
        model.file_encoding = file_enc;
        model
    }

    // -- Versioning ---------------------------------------------------------

    /// Returns the current version id, incremented on each edit.
    pub fn get_version_id(&self) -> u64 {
        self.version_id
    }

    /// Returns the alternative version id that includes undo/redo state.
    pub fn get_alternative_version_id(&self) -> u64 {
        self.alternative_version_id
    }

    /// Create an immutable snapshot of the current model state.
    pub fn create_snapshot(&self) -> ModelSnapshot {
        ModelSnapshot {
            text: self.get_value(),
            version_id: self.version_id,
            line_ending: self.line_ending,
            encoding: self.encoding,
        }
    }

    // -- Line ending --------------------------------------------------------

    /// Get the current line ending mode.
    pub fn get_eol(&self) -> LineEnding {
        self.line_ending
    }

    /// Set the line ending mode (used for subsequent insertions and save).
    pub fn set_eol(&mut self, eol: LineEnding) {
        self.line_ending = eol;
    }

    // -- Encoding -----------------------------------------------------------

    /// Get the current encoding.
    pub fn get_encoding(&self) -> Encoding {
        self.encoding
    }

    /// Set the encoding.
    pub fn set_encoding(&mut self, enc: Encoding) {
        self.encoding = enc;
    }

    /// Get the detected file encoding.
    pub fn get_file_encoding(&self) -> FileEncoding {
        self.file_encoding
    }

    /// Set the file encoding.
    pub fn set_file_encoding(&mut self, enc: FileEncoding) {
        self.file_encoding = enc;
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

    // -- Large file helpers --------------------------------------------------

    /// Create a truncated model showing only the first `max_lines` lines.
    pub fn truncated_model(&self, max_lines: u32) -> TextModel {
        let lc = self.get_line_count();
        if lc <= max_lines {
            return TextModel::new(&self.get_value());
        }
        let end_pos = Position::new(max_lines + 1, 1);
        let end_valid = self.validate_position(end_pos);
        let text = self.get_value_in_range(Range::from_positions(Position::new(1, 1), end_valid));
        TextModel::new(&text)
    }

    // -- Position / offset conversion ----------------------------------------

    /// Convert a byte offset to a 1-based `Position`.
    pub fn offset_to_position(&self, offset: usize) -> Position {
        let offset = offset.min(self.rope.len_bytes());
        let char_idx = self.rope.byte_to_char(offset);
        let line_idx = self.rope.char_to_line(char_idx);
        let line_start_char = self.rope.line_to_char(line_idx);
        let col_chars = char_idx - line_start_char;
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

    /// Internal: apply an edit to the rope without pushing to undo stacks.
    /// Returns the EditOperation and ContentChange.
    fn apply_edit_raw(&mut self, range: Range, text: &str) -> (EditOperation, ContentChange) {
        let range = self.validate_range(range);
        let replaced_text = self.get_value_in_range(range);
        let range_length = replaced_text.len();
        let range_offset = self.position_to_offset(range.start);

        let start_offset = range_offset;
        let end_offset = self.position_to_offset(range.end);
        let start_char = self.rope.byte_to_char(start_offset);
        let end_char = self.rope.byte_to_char(end_offset);

        self.rope.remove(start_char..end_char);
        if !text.is_empty() {
            self.rope.insert(start_char, text);
        }

        // Invalidate line cache
        unsafe {
            (*self.line_cache.get()).0 = 0;
        }

        let end_after = self.offset_to_position(start_offset + text.len());
        let range_after = Range::from_positions(range.start, end_after);

        let edit_op = EditOperation {
            range_after,
            text_inserted: text.to_string(),
            text_replaced: replaced_text,
            range_before: range,
            force_move_markers: false,
        };

        let change = ContentChange {
            range,
            text: text.to_string(),
            range_offset,
            range_length,
        };

        (edit_op, change)
    }

    /// Replace the text in `range` with `text`.
    pub fn apply_edit(&mut self, range: Range, text: &str) {
        let (edit_op, change) = self.apply_edit_raw(range, text);

        self.undo_stack.push(edit_op.clone());
        self.undo_service.push_edit(edit_op, None, None);
        self.version_id += 1;
        self.alternative_version_id += 1;

        let event = ModelContentChangedEvent {
            changes: vec![change],
            version_id: self.version_id,
            is_undo: false,
            is_redo: false,
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

    /// Apply a batch of [`EditOperation`] structs atomically as a single undo
    /// group, with optional cursor state.
    pub fn apply_edits(&mut self, edits: &[EditOperation]) {
        self.push_edit_operations_with_cursor(
            &edits
                .iter()
                .map(|e| (e.range_before, e.text_inserted.clone()))
                .collect::<Vec<_>>(),
            None,
            None,
        );
    }

    /// Apply a batch of edits and push to the undo stack.
    ///
    /// Edits are applied in reverse order (bottom-to-top) so that earlier
    /// ranges remain valid.
    pub fn push_edit_operations(&mut self, edits: &[(Range, String)]) {
        self.push_edit_operations_with_cursor(edits, None, None);
    }

    /// Apply a batch of edits with cursor state, grouped as a single undo
    /// step.
    pub fn push_edit_operations_with_cursor(
        &mut self,
        edits: &[(Range, String)],
        cursor_before: Option<CursorState>,
        cursor_after: Option<CursorState>,
    ) {
        let mut sorted: Vec<(Range, String)> = edits.to_vec();
        sorted.sort_by(|a, b| b.0.start.cmp(&a.0.start));

        self.undo_service.open_group(cursor_before);

        let mut changes = Vec::with_capacity(sorted.len());
        for (range, text) in &sorted {
            let (edit_op, change) = self.apply_edit_raw(*range, text);
            self.undo_stack.push(edit_op.clone());
            self.undo_service.push_edit(edit_op, None, None);
            changes.push(change);
        }

        self.undo_service.close_group(cursor_after);
        self.version_id += 1;
        self.alternative_version_id += 1;

        let event = ModelContentChangedEvent {
            changes,
            version_id: self.version_id,
            is_undo: false,
            is_redo: false,
        };
        self.on_did_change_content_emitter.fire(&event);
    }

    // -- Grouped undo operations --------------------------------------------

    /// Open an undo group. All edits until `close_undo_group` are a single
    /// undo step.
    pub fn open_undo_group(&mut self, cursor_before: Option<CursorState>) {
        self.undo_service.open_group(cursor_before);
    }

    /// Close the current undo group.
    pub fn close_undo_group(&mut self, cursor_after: Option<CursorState>) {
        self.undo_service.close_group(cursor_after);
    }

    // -- Undo / Redo ---------------------------------------------------------

    /// Undo the last edit operation (legacy, returns bool).
    pub fn undo(&mut self) -> bool {
        let op = match self.undo_stack.undo() {
            Some(op) => op,
            None => return false,
        };

        let start_offset = self.position_to_offset(op.range_after.start);
        let end_offset = self.position_to_offset(op.range_after.end);
        let start_char = self.rope.byte_to_char(start_offset);
        let end_char = self.rope.byte_to_char(end_offset);

        self.rope.remove(start_char..end_char);
        if !op.text_replaced.is_empty() {
            self.rope.insert(start_char, &op.text_replaced);
        }
        unsafe {
            (*self.line_cache.get()).0 = 0;
        }

        self.version_id += 1;
        self.alternative_version_id += 1;

        let change = ContentChange {
            range: op.range_after,
            text: op.text_replaced.clone(),
            range_offset: start_offset,
            range_length: op.text_inserted.len(),
        };
        let event = ModelContentChangedEvent {
            changes: vec![change],
            version_id: self.version_id,
            is_undo: true,
            is_redo: false,
        };
        self.on_did_change_content_emitter.fire(&event);
        true
    }

    /// Redo the last undone edit operation (legacy, returns bool).
    pub fn redo(&mut self) -> bool {
        let op = match self.undo_stack.redo() {
            Some(op) => op,
            None => return false,
        };

        let start_offset = self.position_to_offset(op.range_before.start);
        let end_offset = self.position_to_offset(op.range_before.end);
        let start_char = self.rope.byte_to_char(start_offset);
        let end_char = self.rope.byte_to_char(end_offset);

        self.rope.remove(start_char..end_char);
        if !op.text_inserted.is_empty() {
            self.rope.insert(start_char, &op.text_inserted);
        }
        unsafe {
            (*self.line_cache.get()).0 = 0;
        }

        self.version_id += 1;
        self.alternative_version_id += 1;

        let change = ContentChange {
            range: op.range_before,
            text: op.text_inserted.clone(),
            range_offset: start_offset,
            range_length: op.text_replaced.len(),
        };
        let event = ModelContentChangedEvent {
            changes: vec![change],
            version_id: self.version_id,
            is_undo: false,
            is_redo: true,
        };
        self.on_did_change_content_emitter.fire(&event);
        true
    }

    /// Undo using the grouped service, returning cursor state to restore.
    pub fn undo_grouped(&mut self) -> Option<CursorState> {
        let group = self.undo_service.undo()?.clone();
        // Reverse edits in reverse order
        for op in group.edits.iter().rev() {
            let start_offset = self.position_to_offset(op.range_after.start);
            let end_offset = self.position_to_offset(op.range_after.end);
            let start_char = self.rope.byte_to_char(start_offset);
            let end_char = self.rope.byte_to_char(end_offset);
            self.rope.remove(start_char..end_char);
            if !op.text_replaced.is_empty() {
                self.rope.insert(start_char, &op.text_replaced);
            }
        }
        unsafe {
            (*self.line_cache.get()).0 = 0;
        }
        self.version_id += 1;
        self.alternative_version_id += 1;
        group.cursor_before.clone()
    }

    /// Redo using the grouped service, returning cursor state to restore.
    pub fn redo_grouped(&mut self) -> Option<CursorState> {
        let group = self.undo_service.redo()?.clone();
        for op in &group.edits {
            let start_offset = self.position_to_offset(op.range_before.start);
            let end_offset = self.position_to_offset(op.range_before.end);
            let start_char = self.rope.byte_to_char(start_offset);
            let end_char = self.rope.byte_to_char(end_offset);
            self.rope.remove(start_char..end_char);
            if !op.text_inserted.is_empty() {
                self.rope.insert(start_char, &op.text_inserted);
            }
        }
        unsafe {
            (*self.line_cache.get()).0 = 0;
        }
        self.version_id += 1;
        self.alternative_version_id += 1;
        group.cursor_after.clone()
    }

    // -- Search --------------------------------------------------------------

    /// Find all matches of a search string in the document.
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

    fn cache_line(&self, line_number: u32) {
        let cache = unsafe { &mut *self.line_cache.get() };
        if cache.0 == line_number {
            return;
        }
        let line_idx = (line_number - 1) as usize;
        let line_slice = self.rope.line(line_idx);
        let mut s = line_slice.to_string();
        if s.ends_with("\r\n") {
            s.truncate(s.len() - 2);
        } else if s.ends_with('\n') || s.ends_with('\r') {
            s.truncate(s.len() - 1);
        }
        *cache = (line_number, s);
    }
}

// ---------------------------------------------------------------------------
// Encoding — helpers
// ---------------------------------------------------------------------------

impl Encoding {
    /// Return the IANA charset label for the encoding.
    pub fn label(&self) -> &'static str {
        match self {
            Encoding::UTF8 => "utf-8",
            Encoding::UTF8BOM => "utf-8-bom",
            Encoding::UTF16LE => "utf-16le",
            Encoding::UTF16BE => "utf-16be",
            Encoding::Latin1 => "iso-8859-1",
            Encoding::ShiftJIS => "shift_jis",
            Encoding::GBK => "gbk",
        }
    }

    /// Parse an IANA label back to an `Encoding`, case-insensitive.
    pub fn from_label(label: &str) -> Option<Self> {
        match label.to_ascii_lowercase().as_str() {
            "utf-8" | "utf8" => Some(Encoding::UTF8),
            "utf-8-bom" | "utf8bom" => Some(Encoding::UTF8BOM),
            "utf-16le" | "utf16le" => Some(Encoding::UTF16LE),
            "utf-16be" | "utf16be" => Some(Encoding::UTF16BE),
            "iso-8859-1" | "latin1" => Some(Encoding::Latin1),
            "shift_jis" | "shiftjis" => Some(Encoding::ShiftJIS),
            "gbk" => Some(Encoding::GBK),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// FileEncoding — helpers
// ---------------------------------------------------------------------------

impl FileEncoding {
    /// Return the BOM bytes for this encoding (empty for encodings without a BOM).
    pub fn bom_bytes(&self) -> &'static [u8] {
        match self {
            Self::Utf8 | Self::Latin1 => &[],
            Self::Utf8Bom => &[0xEF, 0xBB, 0xBF],
            Self::Utf16Le => &[0xFF, 0xFE],
            Self::Utf16Be => &[0xFE, 0xFF],
        }
    }

    /// Return a human-readable label.
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 with BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Latin1 => "ISO 8859-1",
        }
    }

    /// Convert to the full `Encoding` enum.
    pub fn to_encoding(&self) -> Encoding {
        match self {
            Self::Utf8 => Encoding::UTF8,
            Self::Utf8Bom => Encoding::UTF8BOM,
            Self::Utf16Le => Encoding::UTF16LE,
            Self::Utf16Be => Encoding::UTF16BE,
            Self::Latin1 => Encoding::Latin1,
        }
    }
}

// ---------------------------------------------------------------------------
// LineEnding — helpers
// ---------------------------------------------------------------------------

/// Count individual LF and CRLF occurrences in `text`.
pub fn count_line_endings(text: &str) -> (u32, u32) {
    let mut lf = 0u32;
    let mut crlf = 0u32;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'\r' && i + 1 < len && bytes[i + 1] == b'\n' {
            crlf += 1;
            i += 2;
        } else if bytes[i] == b'\n' {
            lf += 1;
            i += 1;
        } else {
            i += 1;
        }
    }
    (lf, crlf)
}

impl LineEnding {
    /// Detect the predominant line ending and return the corresponding
    /// `LineEnding`. Mixed or no-newline text defaults to LF.
    pub fn detect(text: &str) -> Self {
        match detect_line_ending(text) {
            DetectedLineEnding::CRLF => LineEnding::CRLF,
            _ => LineEnding::LF,
        }
    }
}

// ---------------------------------------------------------------------------
// ContentChange — helpers
// ---------------------------------------------------------------------------

impl ContentChange {
    /// True when the change only inserts text (empty range).
    pub fn is_insert(&self) -> bool {
        self.range_length == 0 && !self.text.is_empty()
    }

    /// True when the change only deletes text (empty replacement).
    pub fn is_delete(&self) -> bool {
        self.range_length > 0 && self.text.is_empty()
    }

    /// True when the change replaces existing text with different text.
    pub fn is_replace(&self) -> bool {
        self.range_length > 0 && !self.text.is_empty()
    }

    /// Net byte-length delta produced by this change.
    pub fn delta(&self) -> isize {
        self.text.len() as isize - self.range_length as isize
    }
}

// ---------------------------------------------------------------------------
// EditOperation — helpers
// ---------------------------------------------------------------------------

impl EditOperation {
    /// True when the edit produces no visible change.
    pub fn is_noop(&self) -> bool {
        self.text_inserted == self.text_replaced
    }

    /// Produce the inverse operation that undoes this edit.
    pub fn inverse(&self) -> Self {
        EditOperation {
            range_after: self.range_before,
            text_inserted: self.text_replaced.clone(),
            text_replaced: self.text_inserted.clone(),
            range_before: self.range_after,
            force_move_markers: self.force_move_markers,
        }
    }
}

// ---------------------------------------------------------------------------
// ModelSnapshot — helpers
// ---------------------------------------------------------------------------

impl ModelSnapshot {
    /// Number of lines in the snapshot.
    pub fn line_count(&self) -> u32 {
        if self.text.is_empty() {
            return 1;
        }
        self.text.split('\n').count() as u32
    }

    /// Return the content of the given 1-based line without the trailing newline.
    pub fn get_line_content(&self, line_number: u32) -> Option<&str> {
        self.text
            .split('\n')
            .nth((line_number - 1) as usize)
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
    }

    /// Simple whitespace-delimited word count.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

// ---------------------------------------------------------------------------
// TextModel — additional queries
// ---------------------------------------------------------------------------

impl TextModel {
    /// Count whitespace-delimited words in the document.
    pub fn get_word_count(&self) -> usize {
        let text = self.get_value();
        text.split_whitespace().count()
    }

    /// Count Unicode scalar values (characters) in the document.
    pub fn get_char_count(&self) -> usize {
        self.rope.len_chars()
    }

    /// Replace all non-overlapping occurrences of `search` with `replacement`.
    /// Returns the number of replacements made.
    pub fn replace_all(&mut self, search: &str, replacement: &str) -> usize {
        if search.is_empty() {
            return 0;
        }
        let matches = self.find_matches(search, false, true);
        let count = matches.len();
        // Apply in reverse so earlier ranges stay valid.
        for range in matches.into_iter().rev() {
            self.apply_edit(range, replacement);
        }
        count
    }

    /// Return true if the document text is empty.
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Return the first line's content.
    pub fn first_line(&self) -> &str {
        self.cache_line(1);
        unsafe { (*self.line_cache.get()).1.as_str() }
    }

    /// Return the last line's content.
    pub fn last_line(&self) -> &str {
        let lc = self.get_line_count();
        self.cache_line(lc);
        unsafe { (*self.line_cache.get()).1.as_str() }
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
// TextModelSnapshot — rich point-in-time copy
// ---------------------------------------------------------------------------

/// A point-in-time copy of document text with pre-computed statistics.
///
/// Unlike [`ModelSnapshot`] (which stores version and encoding metadata),
/// `TextModelSnapshot` focuses on content analysis: line count, word count,
/// character count, and line-level access.
#[derive(Debug, Clone)]
pub struct TextModelSnapshot {
    text: String,
    lines: Vec<String>,
    word_count: usize,
}

impl TextModelSnapshot {
    /// Create a snapshot from raw text.
    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n')
                .map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned())
                .collect()
        };
        let word_count = text.split_whitespace().count();
        Self {
            text: text.to_owned(),
            lines,
            word_count,
        }
    }

    /// The full text of the snapshot.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Number of lines (always ≥ 1).
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Whitespace-delimited word count.
    pub fn word_count(&self) -> usize {
        self.word_count
    }

    /// Return the content of a 1-based line number.
    pub fn get_line(&self, line_num: usize) -> Option<&str> {
        if line_num == 0 || line_num > self.lines.len() {
            None
        } else {
            Some(self.lines[line_num - 1].as_str())
        }
    }

    /// Number of Unicode scalar values (characters).
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Whether the snapshot text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl std::fmt::Display for TextModelSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TextModelSnapshot({} lines, {} words, {} chars)",
            self.line_count(),
            self.word_count(),
            self.char_count(),
        )
    }
}

// ---------------------------------------------------------------------------
// TextModelBracketTracker
// ---------------------------------------------------------------------------

/// Tracks bracket balance across processed lines.
///
/// Supports `()`, `[]`, and `{}`. Characters inside string literals and
/// comments are still counted (a simple, fast approximation).
#[derive(Debug, Clone)]
pub struct TextModelBracketTracker {
    round_open: usize,
    round_close: usize,
    square_open: usize,
    square_close: usize,
    curly_open: usize,
    curly_close: usize,
}

impl TextModelBracketTracker {
    /// Create a new tracker with zero counts.
    pub fn new() -> Self {
        Self {
            round_open: 0,
            round_close: 0,
            square_open: 0,
            square_close: 0,
            curly_open: 0,
            curly_close: 0,
        }
    }

    /// Process a single line, updating bracket counts.
    pub fn process_line(&mut self, line: &str) {
        for ch in line.chars() {
            match ch {
                '(' => self.round_open += 1,
                ')' => {
                    if self.round_open > self.round_close {
                        self.round_close += 1;
                    } else {
                        self.round_close += 1;
                    }
                }
                '[' => self.square_open += 1,
                ']' => {
                    if self.square_open > self.square_close {
                        self.square_close += 1;
                    } else {
                        self.square_close += 1;
                    }
                }
                '{' => self.curly_open += 1,
                '}' => {
                    if self.curly_open > self.curly_close {
                        self.curly_close += 1;
                    } else {
                        self.curly_close += 1;
                    }
                }
                _ => {}
            }
        }
    }

    /// Net nesting depth (opens minus closes), summed over all bracket types.
    pub fn depth(&self) -> i32 {
        let opens = (self.round_open + self.square_open + self.curly_open) as i32;
        let closes = (self.round_close + self.square_close + self.curly_close) as i32;
        opens - closes
    }

    /// `true` if every opened bracket has a matching close.
    pub fn is_balanced(&self) -> bool {
        self.round_open == self.round_close
            && self.square_open == self.square_close
            && self.curly_open == self.curly_close
    }

    /// Number of opening brackets without a matching close.
    pub fn unmatched_open(&self) -> usize {
        self.round_open.saturating_sub(self.round_close)
            + self.square_open.saturating_sub(self.square_close)
            + self.curly_open.saturating_sub(self.curly_close)
    }

    /// Number of closing brackets without a matching open.
    pub fn unmatched_close(&self) -> usize {
        self.round_close.saturating_sub(self.round_open)
            + self.square_close.saturating_sub(self.square_open)
            + self.curly_close.saturating_sub(self.curly_open)
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        self.round_open = 0;
        self.round_close = 0;
        self.square_open = 0;
        self.square_close = 0;
        self.curly_open = 0;
        self.curly_close = 0;
    }
}

impl Default for TextModelBracketTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TextModelBracketTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BracketTracker(depth={}, balanced={})",
            self.depth(),
            self.is_balanced()
        )
    }
}

// ---------------------------------------------------------------------------
// TextModelSearcher / SearchMatch
// ---------------------------------------------------------------------------

/// A single search hit within a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column (byte offset within the line + 1).
    pub column: usize,
    /// Length of the matched text in bytes.
    pub length: usize,
    /// The matched text itself.
    pub text: String,
}

impl std::fmt::Display for SearchMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Match({}:{}, len={}, {:?})",
            self.line, self.column, self.length, self.text
        )
    }
}

/// Multi-mode searcher for text content.
///
/// Provides literal, case-insensitive, and whole-word search modes.
pub struct TextModelSearcher;

impl TextModelSearcher {
    pub fn new() -> Self {
        Self
    }

    /// Find all literal (case-sensitive) occurrences of `query` in `text`.
    pub fn search_literal(&self, text: &str, query: &str) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::new();
        for (line_idx, line) in text.split('\n').enumerate() {
            let line_clean = line.strip_suffix('\r').unwrap_or(line);
            let mut start = 0;
            while let Some(pos) = line_clean[start..].find(query) {
                let col = start + pos;
                results.push(SearchMatch {
                    line: line_idx + 1,
                    column: col + 1,
                    length: query.len(),
                    text: query.to_owned(),
                });
                start = col + query.len();
            }
        }
        results
    }

    /// Find all case-insensitive occurrences of `query` in `text`.
    pub fn search_case_insensitive(&self, text: &str, query: &str) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        for (line_idx, line) in text.split('\n').enumerate() {
            let line_clean = line.strip_suffix('\r').unwrap_or(line);
            let line_lower = line_clean.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                let col = start + pos;
                let matched = &line_clean[col..col + query.len()];
                results.push(SearchMatch {
                    line: line_idx + 1,
                    column: col + 1,
                    length: query.len(),
                    text: matched.to_owned(),
                });
                start = col + query.len();
            }
        }
        results
    }

    /// Find whole-word occurrences of `word` in `text`.
    ///
    /// A "word boundary" is defined as a transition between a word character
    /// (`[A-Za-z0-9_]`) and a non-word character (or start/end of line).
    pub fn search_word(&self, text: &str, word: &str) -> Vec<SearchMatch> {
        if word.is_empty() {
            return Vec::new();
        }
        let pattern = format!(r"\b{}\b", regex::escape(word));
        let re = Regex::new(&pattern).expect("valid regex from escaped word");
        let mut results = Vec::new();
        for (line_idx, line) in text.split('\n').enumerate() {
            let line_clean = line.strip_suffix('\r').unwrap_or(line);
            for m in re.find_iter(line_clean) {
                results.push(SearchMatch {
                    line: line_idx + 1,
                    column: m.start() + 1,
                    length: m.len(),
                    text: m.as_str().to_owned(),
                });
            }
        }
        results
    }

    /// Count the total number of literal matches of `query` in `text`.
    pub fn count_matches(&self, text: &str, query: &str) -> usize {
        self.search_literal(text, query).len()
    }
}

impl Default for TextModelSearcher {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TextModelSearcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TextModelSearcher")
    }
}

// ---------------------------------------------------------------------------
// TextModelEncodingDetector / EncodingGuess / LineEndingKind
// ---------------------------------------------------------------------------

/// Detected line ending style (richer than [`DetectedLineEnding`], includes CR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndingKind {
    /// Unix `\n`.
    LF,
    /// Windows `\r\n`.
    CRLF,
    /// Classic Mac `\r` (no following `\n`).
    CR,
    /// A mix of styles.
    Mixed,
}

impl std::fmt::Display for LineEndingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LineEndingKind::LF => write!(f, "LF"),
            LineEndingKind::CRLF => write!(f, "CRLF"),
            LineEndingKind::CR => write!(f, "CR"),
            LineEndingKind::Mixed => write!(f, "Mixed"),
        }
    }
}

/// Result of encoding detection.
#[derive(Debug, Clone)]
pub struct EncodingGuess {
    /// Name of the detected encoding (e.g. `"UTF-8"`, `"UTF-16LE"`).
    pub encoding: String,
    /// Confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// Whether a BOM was found.
    pub has_bom: bool,
}

impl std::fmt::Display for EncodingGuess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EncodingGuess({}, confidence={:.2}, bom={})",
            self.encoding, self.confidence, self.has_bom
        )
    }
}

/// Heuristic encoding detector for byte buffers.
pub struct TextModelEncodingDetector;

impl TextModelEncodingDetector {
    /// Detect the most likely encoding of `bytes`.
    pub fn detect(bytes: &[u8]) -> EncodingGuess {
        if Self::has_utf8_bom(bytes) {
            return EncodingGuess {
                encoding: "UTF-8-BOM".to_owned(),
                confidence: 1.0,
                has_bom: true,
            };
        }
        if Self::has_utf16_le_bom(bytes) {
            return EncodingGuess {
                encoding: "UTF-16LE".to_owned(),
                confidence: 1.0,
                has_bom: true,
            };
        }
        if Self::has_utf16_be_bom(bytes) {
            return EncodingGuess {
                encoding: "UTF-16BE".to_owned(),
                confidence: 1.0,
                has_bom: true,
            };
        }
        if Self::is_utf8(bytes) {
            EncodingGuess {
                encoding: "UTF-8".to_owned(),
                confidence: 0.95,
                has_bom: false,
            }
        } else {
            EncodingGuess {
                encoding: "Latin-1".to_owned(),
                confidence: 0.5,
                has_bom: false,
            }
        }
    }

    /// Check whether `bytes` are valid UTF-8.
    pub fn is_utf8(bytes: &[u8]) -> bool {
        std::str::from_utf8(bytes).is_ok()
    }

    /// Check for a UTF-8 BOM (`EF BB BF`).
    pub fn has_utf8_bom(bytes: &[u8]) -> bool {
        bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF
    }

    /// Check for a UTF-16 LE BOM (`FF FE`).
    pub fn has_utf16_le_bom(bytes: &[u8]) -> bool {
        bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE
    }

    /// Check for a UTF-16 BE BOM (`FE FF`).
    pub fn has_utf16_be_bom(bytes: &[u8]) -> bool {
        bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF
    }

    /// Detect the line ending style used in `text`.
    pub fn detect_line_ending(text: &str) -> LineEndingKind {
        let mut lf = 0u32;
        let mut crlf = 0u32;
        let mut cr = 0u32;
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        while i < len {
            if bytes[i] == b'\r' {
                if i + 1 < len && bytes[i + 1] == b'\n' {
                    crlf += 1;
                    i += 2;
                } else {
                    cr += 1;
                    i += 1;
                }
            } else if bytes[i] == b'\n' {
                lf += 1;
                i += 1;
            } else {
                i += 1;
            }
        }
        let kinds_present =
            (if lf > 0 { 1 } else { 0 }) + (if crlf > 0 { 1 } else { 0 }) + (if cr > 0 { 1 } else { 0 });
        if kinds_present > 1 {
            LineEndingKind::Mixed
        } else if crlf > 0 {
            LineEndingKind::CRLF
        } else if cr > 0 {
            LineEndingKind::CR
        } else {
            LineEndingKind::LF
        }
    }
}

impl std::fmt::Display for TextModelEncodingDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TextModelEncodingDetector")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


/// Text model configuration manager.
#[derive(Debug, Clone)]
pub struct TextModelConfig {
    entries: Vec<TextModelEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single text model entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TextModelEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl TextModelEntry {
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

impl TextModelConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: TextModelEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&TextModelEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut TextModelEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&TextModelEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&TextModelEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&TextModelEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<TextModelEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Text buffer and line model — extended utilities (yw)
// ---------------------------------------------------------------------------

/// Metric accumulator for text_model operations.
#[derive(Debug, Clone)]
pub struct YwMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YwMetrics {
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

/// Sliding-window rate counter for text_model.
#[derive(Debug, Clone)]
pub struct YwRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YwRateWindow {
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

/// A small LRU-style cache for text_model lookups.
#[derive(Debug, Clone)]
pub struct YwLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YwLruCache {
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
// xa_ extended helpers for text_model
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaTextModelRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaTextModelRingBuf {
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
pub struct XaTextModelCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaTextModelCounter {
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

impl Default for XaTextModelCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 180
// ---------------------------------------------------------------------------

/// Generic object pool `Xc180Pool<T>`.
pub struct Xc180Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc180Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc180PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc180Pool<T> {
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
    pub fn stats(&self) -> Xc180PoolStats {
        Xc180PoolStats {
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

impl<T> Default for Xc180Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc180Scheduler`.
pub struct Xc180Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc180Scheduler {
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

impl Default for Xc180Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_180 hash for the given byte slice.
pub fn xc_180_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_180 convention.
pub fn xc_180_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_108 deepening: state machine + event bus ---

/// States for the Xd108 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd108State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd108State {
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
pub struct Xd108Transition {
    pub from: Xd108State,
    pub to: Xd108State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd108StateMachine {
    current: Xd108State,
    history: Vec<Xd108Transition>,
    step_counter: usize,
}

impl Xd108StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd108State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd108State {
        self.current
    }

    pub fn history(&self) -> &[Xd108Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd108State) -> Result<Xd108State, String> {
        let allowed = match (self.current, target) {
            (Xd108State::Idle, Xd108State::Running) => true,
            (Xd108State::Running, Xd108State::Paused) => true,
            (Xd108State::Running, Xd108State::Done) => true,
            (Xd108State::Paused, Xd108State::Running) => true,
            (Xd108State::Paused, Xd108State::Done) => true,
            (Xd108State::Done, Xd108State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_108: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd108Transition {
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
            "Xd108SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd108State> {
        let prefix = "Xd108SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd108State::Idle),
            "Running" => Some(Xd108State::Running),
            "Paused" => Some(Xd108State::Paused),
            "Done" => Some(Xd108State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd108State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd108 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd108Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd108Event {
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

type Xd108HandlerFn = Box<dyn Fn(&Xd108Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd108EventBus {
    handlers: Vec<(usize, Option<String>, Xd108HandlerFn)>,
    next_id: usize,
    published: Vec<Xd108Event>,
}

impl Xd108EventBus {
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
        F: Fn(&Xd108Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd108Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd108Event) {
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

    pub fn published_events(&self) -> &[Xd108Event] {
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
// xg_32: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg32Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg32Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg32Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_32: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg32Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg32Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg32Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg32Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 179).
pub struct Xh179SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh179SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 221 as u64,
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

/// A compact bit set supporting boolean operations (variant 179).
pub struct Xh179BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh179BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 179).
pub struct Xi179Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi179Deque<T> {
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
pub struct Xi179Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi179Interval {
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

/// A simple interval tree (variant 179).
pub struct Xi179IntervalTree {
    xi_intervals: Vec<Xi179Interval>,
}

impl Xi179IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi179Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi179Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi179Interval) -> Vec<&Xi179Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi179Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi179Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi179Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi179Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi179Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi179Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 179) ---

/// Disjoint set / union-find for crate 179.
pub struct Xj179UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj179UnionFind {
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

const XJ179_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 179.
pub struct Xj179BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj179BTreeNode<K, V>>>,
    len: usize,
}

struct Xj179BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj179BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj179BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ179_BTREE_ORDER - 1
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
        let mid = XJ179_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj179BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj179BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj179BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj179BTreeNode::xj_new_leaf();
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


// --- xk_179 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk179SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk179SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk179DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk179DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_179).
#[derive(Debug, Clone)]
pub struct Xl179Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl179Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_179).
#[derive(Debug, Clone)]
pub struct Xl179SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl179SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm179MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm179MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm179Tokenizer {
    text: String,
}

impl Xm179Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 179.
pub struct Xn179Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn179Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 179 -----

#[derive(Debug, Clone)]
struct Xn179AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn179AvlNode<K, V>>>,
    right: Option<Box<Xn179AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 179.
#[derive(Debug, Clone)]
pub struct Xn179AVL<K, V> {
    root: Option<Box<Xn179AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn179AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn179AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn179AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn179AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn179AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn179AvlNode<K, V>>) -> Box<Xn179AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn179AvlNode<K, V>>) -> Box<Xn179AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn179AvlNode<K, V>>) -> Box<Xn179AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn179AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn179AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn179AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn179AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn179AvlNode<K, V>>) -> &Xn179AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn179AvlNode<K, V>>) -> (Box<Xn179AvlNode<K, V>>, Option<Box<Xn179AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn179AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn179AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn179AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn179AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn179AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn179AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn179AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo179RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo179Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo179RBNode<K, V> {
    key: K,
    value: V,
    color: Xo179Color,
    left: Option<Box<Xo179RBNode<K, V>>>,
    right: Option<Box<Xo179RBNode<K, V>>>,
}

/// A red-black tree map for crate 179.
#[derive(Debug, Clone)]
pub struct Xo179RedBlack<K, V> {
    root: Option<Box<Xo179RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo179RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo179Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo179RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo179RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo179RBNode {
                    key, value, color: Xo179Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo179RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo179Color::Red)
    }

    fn xo_balance(mut h: Box<Xo179RBNode<K, V>>) -> Box<Xo179RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo179Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo179RBNode<K, V>>) -> Box<Xo179RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo179Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo179RBNode<K, V>>) -> Box<Xo179RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo179Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo179RBNode<K, V>>) {
        h.color = Xo179Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo179Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo179Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo179Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo179RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo179RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo179RBNode<K, V>) -> (K, V, Option<Box<Xo179RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo179RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo179Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo179RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo179ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 179.
#[derive(Debug, Clone)]
pub struct Xo179ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo179ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo179#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo179#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 179).
#[derive(Debug)]
pub struct Xp179SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp179Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp179Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp179Node<K, V>>>,
    xp_right: Option<Box<Xp179Node<K, V>>>,
}

impl<K: Ord, V> Xp179Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp179SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp179SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp179Node<K, V>>>, key: &K) -> Option<Box<Xp179Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp179Node<K, V>>) -> Box<Xp179Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp179Node<K, V>>) -> Box<Xp179Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp179Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp179Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp179Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq179Treap ---------------

use std::cmp::Ordering as Xq179Ord;

struct Xq179TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq179TreapNode<K, V>>>,
    right: Option<Box<Xq179TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq179Treap<K, V> {
    root: Option<Box<Xq179TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq179TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_179_size<K, V>(node: &Option<Box<Xq179TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_179_update_size<K, V>(node: &mut Xq179TreapNode<K, V>) {
    node.size = 1 + xq_179_size(&node.left) + xq_179_size(&node.right);
}

fn xq_179_rotate_right<K, V>(mut node: Box<Xq179TreapNode<K, V>>) -> Box<Xq179TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_179_update_size(&mut node);
    left.right = Some(node);
    xq_179_update_size(&mut left);
    left
}

fn xq_179_rotate_left<K, V>(mut node: Box<Xq179TreapNode<K, V>>) -> Box<Xq179TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_179_update_size(&mut node);
    right.left = Some(node);
    xq_179_update_size(&mut right);
    right
}

fn xq_179_insert_node<K: Ord, V>(
    node: Option<Box<Xq179TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq179TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq179TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq179Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq179Ord::Less => {
                let (new_left, old) = xq_179_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_179_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_179_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq179Ord::Greater => {
                let (new_right, old) = xq_179_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_179_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_179_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_179_remove_node<K: Ord, V>(
    node: Option<Box<Xq179TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq179TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq179Ord::Less => {
                let (new_left, old) = xq_179_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_179_update_size(&mut n);
                (Some(n), old)
            }
            Xq179Ord::Greater => {
                let (new_right, old) = xq_179_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_179_update_size(&mut n);
                (Some(n), old)
            }
            Xq179Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_179_rotate_right(n);
                    let (new_right, old) = xq_179_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_179_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_179_rotate_left(n);
                    let (new_left, old) = xq_179_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_179_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_179_find_min<K, V>(node: &Option<Box<Xq179TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_179_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_179_find_max<K, V>(node: &Option<Box<Xq179TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_179_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_179_rank<K: Ord, V>(node: &Option<Box<Xq179TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq179Ord::Less => xq_179_rank(&n.left, key),
            Xq179Ord::Equal => xq_179_size(&n.left),
            Xq179Ord::Greater => 1 + xq_179_size(&n.left) + xq_179_rank(&n.right, key),
        },
    }
}

fn xq_179_kth<K, V>(node: &Option<Box<Xq179TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_179_size(&n.left);
        if k < left_size {
            xq_179_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_179_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_179_in_order<K: Clone, V>(node: &Option<Box<Xq179TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_179_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_179_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq179Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 179 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_179_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq179Ord::Equal => return Some(&n.value),
                Xq179Ord::Less => cur = &n.left,
                Xq179Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_179_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_179_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_179_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_179_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_179_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_179_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_179_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq179VEBTree ---------------

pub struct Xq179VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq179VEBTree>>,
    clusters: Vec<Option<Box<Xq179VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq179VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq179VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq179VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr179KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr179KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr179BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr179KDNode {
    xr_point: Xr179KDPoint,
    xr_left: Option<Box<Xr179KDNode>>,
    xr_right: Option<Box<Xr179KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr179KDTree {
    xr_root: Option<Box<Xr179KDNode>>,
    xr_size: usize,
}

impl Xr179KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr179KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr179KDNode>>,
        point: Xr179KDPoint,
        depth: usize,
    ) -> Box<Xr179KDNode> {
        match node {
            None => Box::new(Xr179KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr179KDPoint) -> Option<Xr179KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr179KDNode>,
        query: &Xr179KDPoint,
        depth: usize,
        best: &mut Xr179KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr179KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr179KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr179KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr179KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr179KDNode>>, pts: &mut Vec<Xr179KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr179KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr179BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr179BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

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
        assert_eq!(evts[0].changes[0].text, " world");
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

    // -- Line ending detection ----------------------------------------------

    #[test]
    fn detect_lf() {
        assert_eq!(detect_line_ending("hello\nworld\n"), DetectedLineEnding::LF);
    }

    #[test]
    fn detect_crlf() {
        assert_eq!(
            detect_line_ending("hello\r\nworld\r\n"),
            DetectedLineEnding::CRLF
        );
    }

    #[test]
    fn detect_mixed() {
        assert_eq!(
            detect_line_ending("hello\nworld\r\n"),
            DetectedLineEnding::Mixed
        );
    }

    #[test]
    fn detect_no_newlines() {
        assert_eq!(detect_line_ending("hello"), DetectedLineEnding::LF);
    }

    #[test]
    fn normalize_to_lf() {
        assert_eq!(
            normalize_line_endings("a\r\nb\r\n", LineEnding::LF),
            "a\nb\n"
        );
    }

    #[test]
    fn normalize_to_crlf() {
        assert_eq!(
            normalize_line_endings("a\nb\n", LineEnding::CRLF),
            "a\r\nb\r\n"
        );
    }

    #[test]
    fn normalize_mixed_to_lf() {
        assert_eq!(
            normalize_line_endings("a\r\nb\nc\r\n", LineEnding::LF),
            "a\nb\nc\n"
        );
    }

    #[test]
    fn model_detects_crlf() {
        let model = TextModel::new("hello\r\nworld\r\n");
        assert_eq!(model.get_eol(), LineEnding::CRLF);
    }

    #[test]
    fn model_set_eol() {
        let mut model = TextModel::new("hello\nworld");
        assert_eq!(model.get_eol(), LineEnding::LF);
        model.set_eol(LineEnding::CRLF);
        assert_eq!(model.get_eol(), LineEnding::CRLF);
    }

    #[test]
    fn line_ending_as_str() {
        assert_eq!(LineEnding::LF.as_str(), "\n");
        assert_eq!(LineEnding::CRLF.as_str(), "\r\n");
    }

    // -- Encoding -----------------------------------------------------------

    #[test]
    fn detect_utf8_bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        assert_eq!(detect_encoding(bytes), Encoding::UTF8BOM);
    }

    #[test]
    fn detect_utf16le_bom() {
        let bytes = b"\xFF\xFEh\x00";
        assert_eq!(detect_encoding(bytes), Encoding::UTF16LE);
    }

    #[test]
    fn detect_utf16be_bom() {
        let bytes = b"\xFE\xFF\x00h";
        assert_eq!(detect_encoding(bytes), Encoding::UTF16BE);
    }

    #[test]
    fn detect_plain_utf8() {
        assert_eq!(detect_encoding(b"hello world"), Encoding::UTF8);
    }

    #[test]
    fn detect_latin1_fallback() {
        let bytes: &[u8] = &[0xFF, 0xFD, 0x80, 0x81];
        assert_eq!(detect_encoding(bytes), Encoding::Latin1);
    }

    #[test]
    fn decode_utf8_bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        assert_eq!(decode_text(bytes, Encoding::UTF8BOM), "hello");
    }

    #[test]
    fn encode_utf8_bom_roundtrip() {
        let text = "hello world";
        let encoded = encode_text(text, Encoding::UTF8BOM);
        assert_eq!(&encoded[..3], &[0xEF, 0xBB, 0xBF]);
        assert_eq!(decode_text(&encoded, Encoding::UTF8BOM), text);
    }

    #[test]
    fn encode_decode_utf16le() {
        let text = "ABC";
        let encoded = encode_text(text, Encoding::UTF16LE);
        assert_eq!(&encoded[..2], &[0xFF, 0xFE]); // BOM
        let decoded = decode_text(&encoded, Encoding::UTF16LE);
        assert_eq!(decoded, text);
    }

    #[test]
    fn encode_decode_utf16be() {
        let text = "ABC";
        let encoded = encode_text(text, Encoding::UTF16BE);
        assert_eq!(&encoded[..2], &[0xFE, 0xFF]); // BOM
        let decoded = decode_text(&encoded, Encoding::UTF16BE);
        assert_eq!(decoded, text);
    }

    #[test]
    fn encode_decode_latin1() {
        let text = "caf";
        let encoded = encode_text(text, Encoding::Latin1);
        let decoded = decode_text(&encoded, Encoding::Latin1);
        assert_eq!(decoded, text);
    }

    #[test]
    fn model_from_bytes_utf8bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        let model = TextModel::from_bytes(bytes);
        assert_eq!(model.get_value(), "hello");
        assert_eq!(model.get_encoding(), Encoding::UTF8BOM);
    }

    // -- Version ID ---------------------------------------------------------

    #[test]
    fn version_id_increments() {
        let mut model = TextModel::new("hello");
        let v1 = model.get_version_id();
        model.insert(Position::new(1, 6), " world");
        let v2 = model.get_version_id();
        assert!(v2 > v1);
    }

    #[test]
    fn alternative_version_id_increments() {
        let mut model = TextModel::new("hello");
        let v1 = model.get_alternative_version_id();
        model.insert(Position::new(1, 6), "!");
        let v2 = model.get_alternative_version_id();
        assert!(v2 > v1);
        model.undo();
        let v3 = model.get_alternative_version_id();
        assert!(v3 > v2);
    }

    // -- Snapshot -----------------------------------------------------------

    #[test]
    fn snapshot_captures_state() {
        let model = TextModel::new("hello");
        let snap = model.create_snapshot();
        assert_eq!(snap.text, "hello");
        assert_eq!(snap.version_id, model.get_version_id());
        assert_eq!(snap.encoding, Encoding::UTF8);
    }

    #[test]
    fn snapshot_immutable_after_edit() {
        let mut model = TextModel::new("hello");
        let snap = model.create_snapshot();
        model.insert(Position::new(1, 6), " world");
        assert_eq!(snap.text, "hello");
        assert_eq!(model.get_value(), "hello world");
    }

    // -- Truncated model ----------------------------------------------------

    #[test]
    fn truncated_model_limits_lines() {
        let model = TextModel::new("a\nb\nc\nd\ne");
        let truncated = model.truncated_model(3);
        assert!(truncated.get_line_count() <= 4);
    }

    #[test]
    fn truncated_model_small_file_unchanged() {
        let model = TextModel::new("a\nb");
        let truncated = model.truncated_model(10);
        assert_eq!(truncated.get_value(), "a\nb");
    }

    // -- Content change event fields ----------------------------------------

    #[test]
    fn event_contains_version_and_flags() {
        let mut model = TextModel::new("abc");
        let events: Arc<Mutex<Vec<ModelContentChangedEvent>>> =
            Arc::new(Mutex::new(Vec::new()));
        let ec = events.clone();
        let _h = model.on_did_change_content().on(move |e| {
            ec.lock().unwrap().push(e.clone());
        });

        model.insert(Position::new(1, 4), "d");
        model.undo();
        model.redo();

        let evts = events.lock().unwrap();
        assert_eq!(evts.len(), 3);
        assert!(!evts[0].is_undo && !evts[0].is_redo);
        assert!(evts[1].is_undo && !evts[1].is_redo);
        assert!(!evts[2].is_undo && evts[2].is_redo);
        // Version ids are monotonically increasing
        assert!(evts[1].version_id > evts[0].version_id);
        assert!(evts[2].version_id > evts[1].version_id);
    }

    #[test]
    fn content_change_has_range_offset() {
        let mut model = TextModel::new("hello\nworld");
        let events: Arc<Mutex<Vec<ModelContentChangedEvent>>> =
            Arc::new(Mutex::new(Vec::new()));
        let ec = events.clone();
        let _h = model.on_did_change_content().on(move |e| {
            ec.lock().unwrap().push(e.clone());
        });

        model.insert(Position::new(2, 1), "X");
        let evts = events.lock().unwrap();
        assert_eq!(evts[0].changes[0].range_offset, 6);
    }

    // -- Grouped undo/redo with cursor state --------------------------------

    #[test]
    fn grouped_undo_redo_with_cursor() {
        let mut model = TextModel::new("hello");
        let cursor_before =
            vsedit_undoredo::CursorState::single(1, 6);
        let cursor_after =
            vsedit_undoredo::CursorState::single(1, 8);

        model.push_edit_operations_with_cursor(
            &[(Range::new(1, 6, 1, 6), "()".to_string())],
            Some(cursor_before.clone()),
            Some(cursor_after.clone()),
        );

        assert_eq!(model.get_value(), "hello()");

        let restored = model.undo_grouped();
        assert_eq!(model.get_value(), "hello");
        assert_eq!(restored, Some(cursor_before));

        let restored = model.redo_grouped();
        assert_eq!(model.get_value(), "hello()");
        assert_eq!(restored, Some(cursor_after));
    }

    #[test]
    fn open_close_undo_group() {
        let mut model = TextModel::new("abc");
        model.open_undo_group(None);
        model.apply_edit(Range::new(1, 4, 1, 4), "d");
        model.apply_edit(Range::new(1, 5, 1, 5), "e");
        model.close_undo_group(None);

        assert_eq!(model.get_value(), "abcde");
        // The grouped service should have one group with multiple edits
        // Undo via legacy still works per-edit
        model.undo();
        assert_eq!(model.get_value(), "abcd");
        model.undo();
        assert_eq!(model.get_value(), "abc");
    }

    // -- EditOperation struct -----------------------------------------------

    #[test]
    fn edit_operation_force_move_markers() {
        let op = EditOperation {
            range_after: Range::new(1, 1, 1, 5),
            text_inserted: "test".into(),
            text_replaced: "".into(),
            range_before: Range::new(1, 1, 1, 1),
            force_move_markers: true,
        };
        assert!(op.force_move_markers);
    }

    // -- apply_edits (batch EditOperation) ----------------------------------

    #[test]
    fn apply_edits_batch() {
        let mut model = TextModel::new("aabbcc");
        let edits = vec![
            EditOperation {
                range_before: Range::new(1, 1, 1, 3),
                range_after: Range::new(1, 1, 1, 3),
                text_inserted: "AA".into(),
                text_replaced: "".into(),
                force_move_markers: false,
            },
            EditOperation {
                range_before: Range::new(1, 5, 1, 7),
                range_after: Range::new(1, 5, 1, 7),
                text_inserted: "CC".into(),
                text_replaced: "".into(),
                force_move_markers: false,
            },
        ];
        model.apply_edits(&edits);
        assert_eq!(model.get_value(), "AAbbCC");
    }

    // -- Large file ---------------------------------------------------------

    #[test]
    fn is_large_file_nonexistent() {
        assert!(!is_large_file(Path::new("/nonexistent/path/file.txt")));
    }

    // -- FileEncoding -------------------------------------------------------

    #[test]
    fn file_encoding_detect_utf8() {
        assert_eq!(FileEncoding::detect(b"hello world"), FileEncoding::Utf8);
    }

    #[test]
    fn file_encoding_detect_utf8_bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        assert_eq!(FileEncoding::detect(bytes), FileEncoding::Utf8Bom);
    }

    #[test]
    fn file_encoding_detect_utf16le() {
        let bytes = b"\xFF\xFEh\x00i\x00";
        assert_eq!(FileEncoding::detect(bytes), FileEncoding::Utf16Le);
    }

    #[test]
    fn file_encoding_detect_utf16be() {
        let bytes = b"\xFE\xFF\x00h\x00i";
        assert_eq!(FileEncoding::detect(bytes), FileEncoding::Utf16Be);
    }

    #[test]
    fn file_encoding_detect_latin1() {
        // Invalid UTF-8 bytes that don't match any BOM → Latin1 fallback.
        let bytes: &[u8] = &[0x80, 0x81, 0xFE, 0xFD];
        assert_eq!(FileEncoding::detect(bytes), FileEncoding::Latin1);
    }

    #[test]
    fn file_encoding_decode_utf8() {
        assert_eq!(FileEncoding::Utf8.decode(b"hello"), "hello");
    }

    #[test]
    fn file_encoding_decode_utf8_bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        assert_eq!(FileEncoding::Utf8Bom.decode(bytes), "hello");
    }

    #[test]
    fn file_encoding_roundtrip_utf8() {
        let text = "hello world";
        let encoded = FileEncoding::Utf8.encode(text);
        let detected = FileEncoding::detect(&encoded);
        assert_eq!(detected, FileEncoding::Utf8);
        assert_eq!(detected.decode(&encoded), text);
    }

    #[test]
    fn file_encoding_roundtrip_utf8_bom() {
        let text = "hello BOM";
        let encoded = FileEncoding::Utf8Bom.encode(text);
        assert_eq!(&encoded[..3], &[0xEF, 0xBB, 0xBF]);
        let detected = FileEncoding::detect(&encoded);
        assert_eq!(detected, FileEncoding::Utf8Bom);
        assert_eq!(detected.decode(&encoded), text);
    }

    #[test]
    fn file_encoding_roundtrip_utf16le() {
        let text = "ABC";
        let encoded = FileEncoding::Utf16Le.encode(text);
        assert_eq!(&encoded[..2], &[0xFF, 0xFE]);
        let detected = FileEncoding::detect(&encoded);
        assert_eq!(detected, FileEncoding::Utf16Le);
        assert_eq!(detected.decode(&encoded), text);
    }

    #[test]
    fn file_encoding_roundtrip_utf16be() {
        let text = "ABC";
        let encoded = FileEncoding::Utf16Be.encode(text);
        assert_eq!(&encoded[..2], &[0xFE, 0xFF]);
        let detected = FileEncoding::detect(&encoded);
        assert_eq!(detected, FileEncoding::Utf16Be);
        assert_eq!(detected.decode(&encoded), text);
    }

    #[test]
    fn file_encoding_roundtrip_latin1() {
        let text = "caf";
        let encoded = FileEncoding::Latin1.encode(text);
        let decoded = FileEncoding::Latin1.decode(&encoded);
        assert_eq!(decoded, text);
    }

    #[test]
    fn file_encoding_detect_empty() {
        assert_eq!(FileEncoding::detect(b""), FileEncoding::Utf8);
    }

    #[test]
    fn model_from_bytes_sets_file_encoding() {
        let bytes = b"\xEF\xBB\xBFhello";
        let model = TextModel::from_bytes(bytes);
        assert_eq!(model.get_file_encoding(), FileEncoding::Utf8Bom);
        assert_eq!(model.get_value(), "hello");
    }

    #[test]
    fn model_new_defaults_to_utf8_file_encoding() {
        let model = TextModel::new("hello");
        assert_eq!(model.get_file_encoding(), FileEncoding::Utf8);
    }

    // -- New functionality tests --------------------------------------------

    #[test]
    fn encoding_label_roundtrip() {
        let encodings = [
            Encoding::UTF8,
            Encoding::UTF8BOM,
            Encoding::UTF16LE,
            Encoding::UTF16BE,
            Encoding::Latin1,
            Encoding::ShiftJIS,
            Encoding::GBK,
        ];
        for enc in &encodings {
            let label = enc.label();
            let parsed = Encoding::from_label(label);
            assert_eq!(parsed, Some(*enc), "roundtrip failed for {:?}", enc);
        }
    }

    #[test]
    fn encoding_from_label_case_insensitive() {
        assert_eq!(Encoding::from_label("UTF-8"), Some(Encoding::UTF8));
        assert_eq!(Encoding::from_label("Utf-16LE"), Some(Encoding::UTF16LE));
        assert_eq!(Encoding::from_label("unknown"), None);
    }

    #[test]
    fn file_encoding_bom_bytes() {
        assert!(FileEncoding::Utf8.bom_bytes().is_empty());
        assert_eq!(FileEncoding::Utf8Bom.bom_bytes(), &[0xEF, 0xBB, 0xBF]);
        assert_eq!(FileEncoding::Utf16Le.bom_bytes(), &[0xFF, 0xFE]);
        assert_eq!(FileEncoding::Utf16Be.bom_bytes(), &[0xFE, 0xFF]);
        assert!(FileEncoding::Latin1.bom_bytes().is_empty());
    }

    #[test]
    fn file_encoding_display_label() {
        assert_eq!(FileEncoding::Utf8.display_label(), "UTF-8");
        assert_eq!(FileEncoding::Utf8Bom.display_label(), "UTF-8 with BOM");
        assert_eq!(FileEncoding::Latin1.display_label(), "ISO 8859-1");
    }

    #[test]
    fn file_encoding_to_encoding() {
        assert_eq!(FileEncoding::Utf8.to_encoding(), Encoding::UTF8);
        assert_eq!(FileEncoding::Utf8Bom.to_encoding(), Encoding::UTF8BOM);
        assert_eq!(FileEncoding::Utf16Le.to_encoding(), Encoding::UTF16LE);
        assert_eq!(FileEncoding::Utf16Be.to_encoding(), Encoding::UTF16BE);
        assert_eq!(FileEncoding::Latin1.to_encoding(), Encoding::Latin1);
    }

    #[test]
    fn count_line_endings_basic() {
        assert_eq!(count_line_endings("a\nb\nc\n"), (3, 0));
        assert_eq!(count_line_endings("a\r\nb\r\n"), (0, 2));
        assert_eq!(count_line_endings("a\nb\r\n"), (1, 1));
        assert_eq!(count_line_endings("no newlines"), (0, 0));
    }

    #[test]
    fn line_ending_detect_helper() {
        assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::CRLF);
        assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::LF);
        assert_eq!(LineEnding::detect("no newlines"), LineEnding::LF);
    }

    #[test]
    fn content_change_classification() {
        let insert = ContentChange {
            range: Range::new(1, 1, 1, 1),
            text: "hello".into(),
            range_offset: 0,
            range_length: 0,
        };
        assert!(insert.is_insert());
        assert!(!insert.is_delete());
        assert!(!insert.is_replace());
        assert_eq!(insert.delta(), 5);

        let delete = ContentChange {
            range: Range::new(1, 1, 1, 6),
            text: String::new(),
            range_offset: 0,
            range_length: 5,
        };
        assert!(!delete.is_insert());
        assert!(delete.is_delete());
        assert!(!delete.is_replace());
        assert_eq!(delete.delta(), -5);

        let replace = ContentChange {
            range: Range::new(1, 1, 1, 4),
            text: "ab".into(),
            range_offset: 0,
            range_length: 3,
        };
        assert!(!replace.is_insert());
        assert!(!replace.is_delete());
        assert!(replace.is_replace());
        assert_eq!(replace.delta(), -1);
    }

    #[test]
    fn edit_operation_is_noop() {
        let noop = EditOperation {
            range_after: Range::new(1, 1, 1, 4),
            text_inserted: "abc".into(),
            text_replaced: "abc".into(),
            range_before: Range::new(1, 1, 1, 4),
            force_move_markers: false,
        };
        assert!(noop.is_noop());

        let real = EditOperation {
            range_after: Range::new(1, 1, 1, 4),
            text_inserted: "xyz".into(),
            text_replaced: "abc".into(),
            range_before: Range::new(1, 1, 1, 4),
            force_move_markers: false,
        };
        assert!(!real.is_noop());
    }

    #[test]
    fn edit_operation_inverse() {
        let op = EditOperation {
            range_after: Range::new(1, 1, 1, 6),
            text_inserted: "hello".into(),
            text_replaced: "hi".into(),
            range_before: Range::new(1, 1, 1, 3),
            force_move_markers: false,
        };
        let inv = op.inverse();
        assert_eq!(inv.text_inserted, "hi");
        assert_eq!(inv.text_replaced, "hello");
        assert_eq!(inv.range_before, op.range_after);
        assert_eq!(inv.range_after, op.range_before);
    }

    #[test]
    fn snapshot_line_count_and_content() {
        let model = TextModel::new("hello\nworld\nfoo");
        let snap = model.create_snapshot();
        assert_eq!(snap.line_count(), 3);
        assert_eq!(snap.get_line_content(1), Some("hello"));
        assert_eq!(snap.get_line_content(2), Some("world"));
        assert_eq!(snap.get_line_content(3), Some("foo"));
        assert_eq!(snap.get_line_content(4), None);
    }

    #[test]
    fn snapshot_word_count() {
        let model = TextModel::new("hello world\nfoo bar baz");
        let snap = model.create_snapshot();
        assert_eq!(snap.word_count(), 5);
    }

    #[test]
    fn model_word_count() {
        let model = TextModel::new("the quick brown fox");
        assert_eq!(model.get_word_count(), 4);
    }

    #[test]
    fn model_char_count() {
        let model = TextModel::new("hello");
        assert_eq!(model.get_char_count(), 5);
    }

    #[test]
    fn model_replace_all() {
        let mut model = TextModel::new("foo bar foo baz foo");
        let count = model.replace_all("foo", "qux");
        assert_eq!(count, 3);
        assert_eq!(model.get_value(), "qux bar qux baz qux");
    }

    #[test]
    fn model_replace_all_no_match() {
        let mut model = TextModel::new("hello world");
        let count = model.replace_all("xyz", "abc");
        assert_eq!(count, 0);
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn model_replace_all_empty_search() {
        let mut model = TextModel::new("hello");
        let count = model.replace_all("", "x");
        assert_eq!(count, 0);
        assert_eq!(model.get_value(), "hello");
    }

    #[test]
    fn model_is_empty() {
        let model = TextModel::new("");
        assert!(model.is_empty());
        let model = TextModel::new("a");
        assert!(!model.is_empty());
    }

    #[test]
    fn model_first_and_last_line() {
        let model = TextModel::new("first\nmiddle\nlast");
        assert_eq!(model.first_line(), "first");
        assert_eq!(model.last_line(), "last");
    }

    #[test]
    fn model_first_last_single_line() {
        let model = TextModel::new("only");
        assert_eq!(model.first_line(), "only");
        assert_eq!(model.last_line(), "only");
    }

    // -- TextModelSnapshot ---------------------------------------------------

    #[test]
    fn text_model_snapshot_from_text() {
        let snap = TextModelSnapshot::from_text("hello world\nfoo bar baz");
        assert_eq!(snap.line_count(), 2);
        assert_eq!(snap.word_count(), 5);
        assert_eq!(snap.get_line(1), Some("hello world"));
        assert_eq!(snap.get_line(2), Some("foo bar baz"));
        assert_eq!(snap.get_line(3), None);
        assert_eq!(snap.char_count(), 23);
        assert!(!snap.is_empty());
    }

    #[test]
    fn text_model_snapshot_empty() {
        let snap = TextModelSnapshot::from_text("");
        assert!(snap.is_empty());
        assert_eq!(snap.line_count(), 1);
        assert_eq!(snap.word_count(), 0);
        assert_eq!(snap.char_count(), 0);
        assert_eq!(snap.get_line(1), Some(""));
    }

    #[test]
    fn text_model_snapshot_display() {
        let snap = TextModelSnapshot::from_text("a b c");
        let display = format!("{}", snap);
        assert!(display.contains("1 lines"));
        assert!(display.contains("3 words"));
    }

    // -- TextModelBracketTracker ---------------------------------------------

    #[test]
    fn bracket_tracker_balanced() {
        let mut tracker = TextModelBracketTracker::new();
        tracker.process_line("fn main() { let v = vec![1, 2]; }");
        assert!(tracker.is_balanced());
        assert_eq!(tracker.depth(), 0);
    }

    #[test]
    fn bracket_tracker_unbalanced() {
        let mut tracker = TextModelBracketTracker::new();
        tracker.process_line("fn foo() {");
        assert!(!tracker.is_balanced());
        assert_eq!(tracker.unmatched_open(), 1);
        assert_eq!(tracker.unmatched_close(), 0);
    }

    #[test]
    fn bracket_tracker_reset() {
        let mut tracker = TextModelBracketTracker::new();
        tracker.process_line("((()))");
        assert!(tracker.is_balanced());
        tracker.reset();
        assert_eq!(tracker.depth(), 0);
        assert!(tracker.is_balanced());
    }

    // -- TextModelSearcher ---------------------------------------------------

    #[test]
    fn searcher_literal() {
        let s = TextModelSearcher::new();
        let results = s.search_literal("hello world\nhello again", "hello");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line, 1);
        assert_eq!(results[0].column, 1);
        assert_eq!(results[1].line, 2);
    }

    #[test]
    fn searcher_case_insensitive() {
        let s = TextModelSearcher::new();
        let results = s.search_case_insensitive("Hello HELLO hello", "hello");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].text, "Hello");
        assert_eq!(results[1].text, "HELLO");
    }

    #[test]
    fn searcher_word_boundary() {
        let s = TextModelSearcher::new();
        let results = s.search_word("the theorem is there", "the");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].column, 1);
    }

    #[test]
    fn searcher_count_matches() {
        let s = TextModelSearcher::new();
        assert_eq!(s.count_matches("aaa", "a"), 3);
        assert_eq!(s.count_matches("aaa", "aa"), 1);
    }

    // -- TextModelEncodingDetector -------------------------------------------

    #[test]
    fn encoding_detect_utf8_bom() {
        let bytes = b"\xEF\xBB\xBFhello";
        let guess = TextModelEncodingDetector::detect(bytes);
        assert_eq!(guess.encoding, "UTF-8-BOM");
        assert!(guess.has_bom);
        assert!((guess.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn encoding_detect_plain_utf8() {
        let guess = TextModelEncodingDetector::detect(b"hello world");
        assert_eq!(guess.encoding, "UTF-8");
        assert!(!guess.has_bom);
    }

    #[test]
    fn encoding_line_ending_detection() {
        assert_eq!(
            TextModelEncodingDetector::detect_line_ending("a\nb\nc"),
            LineEndingKind::LF
        );
        assert_eq!(
            TextModelEncodingDetector::detect_line_ending("a\r\nb\r\n"),
            LineEndingKind::CRLF
        );
        assert_eq!(
            TextModelEncodingDetector::detect_line_ending("a\rb\r"),
            LineEndingKind::CR
        );
        assert_eq!(
            TextModelEncodingDetector::detect_line_ending("a\nb\r\n"),
            LineEndingKind::Mixed
        );
    }

    #[test]
    fn snapshot_empty_document() {
        let model = TextModel::new("");
        let snap = model.create_snapshot();
        assert_eq!(snap.line_count(), 1);
        assert_eq!(snap.word_count(), 0);
        assert_eq!(snap.get_line_content(1), Some(""));
    }

    #[test]
    fn text_model_entry_creation() {
        let e = TextModelEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn text_model_entry_with_priority() {
        let e = TextModelEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn text_model_entry_metadata() {
        let e = TextModelEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn text_model_entry_remove_meta() {
        let mut e = TextModelEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn text_model_entry_activate_deactivate() {
        let mut e = TextModelEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn text_model_config_add_sorted() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("lo", "Lo").with_priority(1));
        c.add(TextModelEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn text_model_config_capacity() {
        let mut c = TextModelConfig::new(1);
        assert!(c.add(TextModelEntry::new("a", "A")));
        assert!(!c.add(TextModelEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn text_model_config_remove() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn text_model_config_get() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn text_model_config_active_entries() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "A"));
        c.add(TextModelEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn text_model_config_enable_disable() {
        let mut c = TextModelConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn text_model_config_clear() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn text_model_config_find_by_label() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn text_model_config_top_n() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "A").with_priority(1));
        c.add(TextModelEntry::new("b", "B").with_priority(2));
        c.add(TextModelEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn text_model_config_deactivate_activate_all() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "A"));
        c.add(TextModelEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn text_model_config_highest_priority() {
        let mut c = TextModelConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(TextModelEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn text_model_config_contains() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn text_model_config_labels() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "Alpha"));
        c.add(TextModelEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn text_model_config_drain_inactive() {
        let mut c = TextModelConfig::new(10);
        c.add(TextModelEntry::new("a", "A"));
        c.add(TextModelEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn yw_metrics_empty() {
        let m = YwMetrics::new("text_model");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yw_metrics_record_and_mean() {
        let mut m = YwMetrics::new("text_model");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yw_metrics_min_max() {
        let mut m = YwMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yw_metrics_variance_and_std() {
        let mut m = YwMetrics::new("v");
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
    fn yw_metrics_percentile() {
        let mut m = YwMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yw_metrics_merge() {
        let mut a = YwMetrics::new("a");
        a.record(1.0);
        let mut b = YwMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yw_metrics_reset() {
        let mut m = YwMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yw_rate_window_empty() {
        let rw = YwRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yw_rate_window_tick_and_rate() {
        let mut rw = YwRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yw_lru_cache_basic() {
        let mut c = YwLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yw_lru_cache_contains_and_keys() {
        let mut c = YwLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yw_lru_cache_remove() {
        let mut c = YwLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yw_metrics_sum() {
        let mut m = YwMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yw_metrics_label() {
        let m = YwMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yw_lru_cache_clear() {
        let mut c = YwLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for text_model
    #[test]
    fn xa_text_model_ring_new() {
        let rb = super::XaTextModelRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_text_model_ring_push_len() {
        let mut rb = super::XaTextModelRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_text_model_ring_wrap() {
        let mut rb = super::XaTextModelRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_text_model_ring_mean_empty() {
        let rb = super::XaTextModelRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_text_model_ring_mean_values() {
        let mut rb = super::XaTextModelRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_text_model_ring_min_max() {
        let mut rb = super::XaTextModelRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_text_model_ring_iter() {
        let mut rb = super::XaTextModelRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_text_model_counter_new() {
        let c = super::XaTextModelCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_text_model_counter_inc() {
        let mut c = super::XaTextModelCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_text_model_counter_inc_by() {
        let mut c = super::XaTextModelCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_text_model_counter_reset() {
        let mut c = super::XaTextModelCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_text_model_counter_clear() {
        let mut c = super::XaTextModelCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_text_model_counter_default() {
        let c = super::XaTextModelCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 180 ----

    #[test]
    fn xc_180_pool_new_empty() {
        let pool: super::Xc180Pool<i32> = super::Xc180Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_180_pool_release_acquire() {
        let mut pool = super::Xc180Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_180_pool_acquire_empty() {
        let mut pool: super::Xc180Pool<i32> = super::Xc180Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_180_pool_full() {
        let mut pool = super::Xc180Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_180_pool_drain() {
        let mut pool = super::Xc180Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_180_pool_stats() {
        let mut pool = super::Xc180Pool::new(8);
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
    fn xc_180_pool_clear() {
        let mut pool = super::Xc180Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_180_pool_shrink() {
        let mut pool = super::Xc180Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_180_pool_default() {
        let pool: super::Xc180Pool<String> = super::Xc180Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_180_pool_extend() {
        let mut pool = super::Xc180Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_180_pool_retain() {
        let mut pool = super::Xc180Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_180_scheduler_round_robin() {
        let mut sched = super::Xc180Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_180_scheduler_empty() {
        let mut sched = super::Xc180Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_180_scheduler_reset() {
        let mut sched = super::Xc180Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_180_scheduler_add_remove() {
        let mut sched = super::Xc180Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_180_scheduler_targets() {
        let sched = super::Xc180Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_180_hash_empty() {
        assert_eq!(super::xc_180_hash(b""), 5381);
    }

    #[test]
    fn xc_180_hash_data() {
        let h = super::xc_180_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_180_hash(b"hello"), h);
    }

    #[test]
    fn xc_180_reverse_str() {
        assert_eq!(super::xc_180_reverse("abc"), "cba");
        assert_eq!(super::xc_180_reverse(""), "");
    }


    // --- xd_108 deepening tests ---

    #[test]
    fn xd_108_sm_initial_state() {
        let sm = Xd108StateMachine::new();
        assert_eq!(sm.current_state(), Xd108State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_108_sm_valid_idle_to_running() {
        let mut sm = Xd108StateMachine::new();
        assert!(sm.transition(Xd108State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd108State::Running);
    }

    #[test]
    fn xd_108_sm_valid_running_to_paused() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        assert!(sm.transition(Xd108State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd108State::Paused);
    }

    #[test]
    fn xd_108_sm_valid_running_to_done() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        assert!(sm.transition(Xd108State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd108State::Done);
    }

    #[test]
    fn xd_108_sm_valid_paused_to_running() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        sm.transition(Xd108State::Paused).unwrap();
        assert!(sm.transition(Xd108State::Running).is_ok());
    }

    #[test]
    fn xd_108_sm_valid_done_to_idle() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        sm.transition(Xd108State::Done).unwrap();
        assert!(sm.transition(Xd108State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd108State::Idle);
    }

    #[test]
    fn xd_108_sm_invalid_idle_to_done() {
        let mut sm = Xd108StateMachine::new();
        assert!(sm.transition(Xd108State::Done).is_err());
    }

    #[test]
    fn xd_108_sm_invalid_idle_to_paused() {
        let mut sm = Xd108StateMachine::new();
        assert!(sm.transition(Xd108State::Paused).is_err());
    }

    #[test]
    fn xd_108_sm_history_tracking() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        sm.transition(Xd108State::Paused).unwrap();
        sm.transition(Xd108State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd108State::Idle);
        assert_eq!(sm.history()[0].to, Xd108State::Running);
        assert_eq!(sm.history()[1].from, Xd108State::Running);
        assert_eq!(sm.history()[2].to, Xd108State::Done);
    }

    #[test]
    fn xd_108_sm_serialize_deserialize() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd108StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd108State::Running));
    }

    #[test]
    fn xd_108_sm_deserialize_invalid() {
        assert_eq!(Xd108StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_108_sm_reset() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd108State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_108_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd108EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd108Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_108_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd108EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd108Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd108Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_108_bus_unsubscribe() {
        let mut bus = Xd108EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_108_event_kind_and_payload() {
        let e = Xd108Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd108Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_108_bus_clear_history() {
        let mut bus = Xd108EventBus::new();
        bus.publish(Xd108Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_108_sm_step_counter_increments() {
        let mut sm = Xd108StateMachine::new();
        sm.transition(Xd108State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd108State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_32 graph tests ------------------------------------------------

    #[test]
    fn xg_32_graph_empty() {
        let g = super::Xg32Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_32_graph_add_node() {
        let mut g = super::Xg32Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_32_graph_add_edge() {
        let mut g = super::Xg32Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_32_graph_neighbors() {
        let mut g = super::Xg32Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_32_graph_has_path() {
        let mut g = super::Xg32Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_32_graph_self_path() {
        let g = super::Xg32Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_32_graph_topo_sort() {
        let mut g = super::Xg32Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_32_graph_cycle_detect_false() {
        let mut g = super::Xg32Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_32_graph_cycle_detect_true() {
        let mut g = super::Xg32Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_32 heap tests -------------------------------------------------

    #[test]
    fn xg_32_heap_empty() {
        let h: super::Xg32Heap<i32> = super::Xg32Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_32_heap_push_pop() {
        let mut h = super::Xg32Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_32_heap_peek() {
        let mut h = super::Xg32Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_32_heap_drain_sorted() {
        let mut h = super::Xg32Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_32_heap_merge() {
        let mut a = super::Xg32Heap::new();
        let mut b = super::Xg32Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_32_heap_default() {
        let h: super::Xg32Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_32_graph_default() {
        let g: super::Xg32Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh179_skip_insert_contains() {
        let mut sl = super::Xh179SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh179_skip_remove() {
        let mut sl = super::Xh179SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh179_skip_len() {
        let mut sl = super::Xh179SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh179_skip_range_query() {
        let mut sl = super::Xh179SkipList::xh_new(4);
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
    fn xh179_skip_floor_ceiling() {
        let mut sl = super::Xh179SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh179_skip_rank() {
        let mut sl = super::Xh179SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh179_skip_empty() {
        let sl = super::Xh179SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh179_skip_duplicates() {
        let mut sl = super::Xh179SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh179_bitset_set_test() {
        let mut bs = super::Xh179BitSet::xh_new(256);
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
    fn xh179_bitset_clear_count() {
        let mut bs = super::Xh179BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh179_bitset_and_or_xor() {
        let mut a = super::Xh179BitSet::xh_new(128);
        let mut b = super::Xh179BitSet::xh_new(128);
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
    fn xh179_bitset_iter_ones() {
        let mut bs = super::Xh179BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh179_bitset_first_last() {
        let mut bs = super::Xh179BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh179_bitset_empty() {
        let bs = super::Xh179BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi179_deque_push_pop_back() {
        let mut dq = super::Xi179Deque::xi_new(4);
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
    fn xi179_deque_push_pop_front() {
        let mut dq = super::Xi179Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi179_deque_mixed_ops() {
        let mut dq = super::Xi179Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi179_deque_get_and_split() {
        let mut dq = super::Xi179Deque::xi_new(8);
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
    fn xi179_deque_rotate_left() {
        let mut dq = super::Xi179Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi179_deque_rotate_right() {
        let mut dq = super::Xi179Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi179_deque_grow() {
        let mut dq = super::Xi179Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi179_deque_empty() {
        let dq = super::Xi179Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi179_interval_tree_insert_query() {
        let mut tree = super::Xi179IntervalTree::xi_new();
        tree.xi_insert(super::Xi179Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi179Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi179Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi179_interval_tree_overlap() {
        let mut tree = super::Xi179IntervalTree::xi_new();
        tree.xi_insert(super::Xi179Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi179Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi179Interval::xi_new(12, 20));
        let q = super::Xi179Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi179_interval_tree_remove() {
        let mut tree = super::Xi179IntervalTree::xi_new();
        tree.xi_insert(super::Xi179Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi179Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi179_interval_tree_gaps() {
        let mut tree = super::Xi179IntervalTree::xi_new();
        tree.xi_insert(super::Xi179Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi179Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi179Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi179Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi179Interval::xi_new(8, 10));
    }

    #[test]
    fn xi179_interval_tree_merge() {
        let mut tree = super::Xi179IntervalTree::xi_new();
        tree.xi_insert(super::Xi179Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi179Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi179Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi179Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi179Interval::xi_new(10, 15));
    }

    #[test]
    fn xi179_interval_tree_all() {
        let mut tree = super::Xi179IntervalTree::xi_new();
        tree.xi_insert(super::Xi179Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi179Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi179_interval_tree_empty() {
        let tree = super::Xi179IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi179_interval_tree_contains_point() {
        let iv = super::Xi179Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 179) ---

    #[test]
    fn xj_179_uf_make_and_find() {
        let mut uf = super::Xj179UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_179_uf_union_connected() {
        let mut uf = super::Xj179UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_179_uf_component_count() {
        let mut uf = super::Xj179UnionFind::xj_new();
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
    fn xj_179_uf_component_size() {
        let mut uf = super::Xj179UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_179_uf_largest_component() {
        let mut uf = super::Xj179UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_179_uf_many_elements() {
        let mut uf = super::Xj179UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_179_uf_separate_components() {
        let mut uf = super::Xj179UnionFind::xj_new();
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
    fn xj_179_uf_path_compression() {
        let mut uf = super::Xj179UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_179_bt_insert_get() {
        let mut bt = super::Xj179BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_179_bt_contains_len() {
        let mut bt = super::Xj179BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_179_bt_replace() {
        let mut bt = super::Xj179BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_179_bt_remove() {
        let mut bt = super::Xj179BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_179_bt_keys_values() {
        let mut bt = super::Xj179BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_179_bt_range() {
        let mut bt = super::Xj179BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_179_bt_min_max() {
        let mut bt = super::Xj179BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_179_bt_many_inserts() {
        let mut bt = super::Xj179BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_179 segment tree tests ---

    #[test]
    fn xk_179_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk179SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_179_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk179SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_179_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk179SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_179_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk179SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_179_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk179SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_179_st_single_element() {
        let data = vec![42];
        let st = super::Xk179SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_179_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk179SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_179_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk179SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_179 disjoint intervals tests ---

    #[test]
    fn xk_179_di_add_and_count() {
        let mut di = super::Xk179DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_179_di_merge_overlap() {
        let mut di = super::Xk179DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_179_di_contains() {
        let mut di = super::Xk179DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_179_di_remove() {
        let mut di = super::Xk179DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_179_di_covered_length() {
        let mut di = super::Xk179DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_179_di_gaps() {
        let mut di = super::Xk179DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_179_di_merge_adjacent() {
        let mut di = super::Xk179DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_179_di_empty() {
        let di = super::Xk179DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_179_rope_new_empty() {
        let rope = super::Xl179Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_179_rope_from_str() {
        let rope = super::Xl179Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_179_rope_insert_at() {
        let mut rope = super::Xl179Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_179_rope_delete_range() {
        let mut rope = super::Xl179Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_179_rope_char_at() {
        let rope = super::Xl179Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_179_rope_split_concat() {
        let rope = super::Xl179Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_179_rope_line_count() {
        let rope = super::Xl179Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_179_rope_line_at() {
        let rope = super::Xl179Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_179_sa_build_and_search() {
        let sa = super::Xl179SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_179_sa_count() {
        let sa = super::Xl179SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_179_sa_longest_repeated() {
        let sa = super::Xl179SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_179_sa_all_positions() {
        let sa = super::Xl179SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_179_sa_len() {
        let sa = super::Xl179SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_179_sa_empty() {
        let sa = super::Xl179SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_179_rope_slice() {
        let rope = super::Xl179Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_179_sa_search_start() {
        let sa = super::Xl179SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_179_sparse_set_get() {
        let mut m = super::Xm179MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_179_sparse_row_col() {
        let mut m = super::Xm179MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_179_sparse_transpose() {
        let mut m = super::Xm179MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_179_sparse_multiply_vec() {
        let mut m = super::Xm179MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_179_sparse_nnz_density() {
        let mut m = super::Xm179MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_179_sparse_clear() {
        let mut m = super::Xm179MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_179_sparse_overwrite_zero() {
        let mut m = super::Xm179MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_179_tokenizer_basic() {
        let t = super::Xm179Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_179_tokenizer_count() {
        let t = super::Xm179Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_179_tokenizer_unique() {
        let t = super::Xm179Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_179_tokenizer_frequency() {
        let t = super::Xm179Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_179_tokenizer_delimiter() {
        let t = super::Xm179Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_179_tokenizer_whitespace() {
        let t = super::Xm179Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_179_tokenizer_empty() {
        let t = super::Xm179Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 179 ----

    #[test]
    fn xn_179_fenwick_prefix_sum() {
        let mut ft = super::Xn179Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_179_fenwick_range_sum() {
        let mut ft = super::Xn179Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_179_fenwick_point_query() {
        let mut ft = super::Xn179Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_179_fenwick_len() {
        let ft = super::Xn179Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_179_fenwick_multiple_updates() {
        let mut ft = super::Xn179Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_179_fenwick_single_element() {
        let mut ft = super::Xn179Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_179_fenwick_find_kth() {
        let mut ft = super::Xn179Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_179_fenwick_negative_delta() {
        let mut ft = super::Xn179Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 179 ----

    #[test]
    fn xn_179_avl_insert_get() {
        let mut m = super::Xn179AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_179_avl_remove() {
        let mut m = super::Xn179AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_179_avl_in_order() {
        let mut m = super::Xn179AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_179_avl_min_max() {
        let mut m = super::Xn179AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_179_avl_floor_ceiling() {
        let mut m = super::Xn179AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_179_avl_height_balanced() {
        let mut m = super::Xn179AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_179_avl_overwrite() {
        let mut m = super::Xn179AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_179_avl_empty() {
        let m: super::Xn179AVL<i32, i32> = super::Xn179AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo179RedBlack tests ---

    #[test]
    fn xo_179_rb_insert_and_get() {
        let mut tree = super::Xo179RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_179_rb_len_and_empty() {
        let mut tree = super::Xo179RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_179_rb_min_max() {
        let mut tree = super::Xo179RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_179_rb_contains() {
        let mut tree = super::Xo179RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_179_rb_remove() {
        let mut tree = super::Xo179RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_179_rb_in_order() {
        let mut tree = super::Xo179RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_179_rb_black_height() {
        let mut tree = super::Xo179RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_179_rb_overwrite() {
        let mut tree = super::Xo179RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo179ConsistentHash tests ---

    #[test]
    fn xo_179_ch_add_and_count() {
        let mut ring = super::Xo179ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_179_ch_remove_node() {
        let mut ring = super::Xo179ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_179_ch_get_node() {
        let mut ring = super::Xo179ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_179_ch_empty_ring() {
        let ring = super::Xo179ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_179_ch_distribution() {
        let mut ring = super::Xo179ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_179_ch_rebalance() {
        let mut ring = super::Xo179ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_179_ch_virtual_nodes() {
        let mut ring = super::Xo179ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_179_ch_consistent_lookup() {
        let mut ring = super::Xo179ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_179_splay_insert_get() {
        let mut t = super::Xp179SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_179_splay_remove() {
        let mut t = super::Xp179SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_179_splay_count_increases() {
        let mut t = super::Xp179SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_179_splay_depth() {
        let mut t = super::Xp179SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_179_splay_len_empty() {
        let t = super::Xp179SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_179_splay_min_max() {
        let mut t = super::Xp179SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_179_splay_overwrite() {
        let mut t = super::Xp179SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_179_splay_remove_missing() {
        let mut t = super::Xp179SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_179 treap tests ----
    #[test]
    fn xq_179_treap_empty() {
        let t = super::Xq179Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_179_treap_insert_get() {
        let mut t = super::Xq179Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_179_treap_overwrite() {
        let mut t = super::Xq179Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_179_treap_remove() {
        let mut t = super::Xq179Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_179_treap_min_max() {
        let mut t = super::Xq179Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_179_treap_rank() {
        let mut t = super::Xq179Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_179_treap_kth() {
        let mut t = super::Xq179Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_179_treap_in_order() {
        let mut t = super::Xq179Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_179 VEB tree tests ----
    #[test]
    fn xq_179_veb_empty() {
        let v = super::Xq179VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_179_veb_insert_contains() {
        let mut v = super::Xq179VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_179_veb_min_max() {
        let mut v = super::Xq179VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_179_veb_delete() {
        let mut v = super::Xq179VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_179_veb_successor() {
        let mut v = super::Xq179VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_179_veb_predecessor() {
        let mut v = super::Xq179VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_179_veb_count() {
        let mut v = super::Xq179VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_179_veb_duplicate_insert() {
        let mut v = super::Xq179VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_179_kdtree_empty() {
        let tree = super::Xr179KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_179_kdtree_insert_one() {
        let mut tree = super::Xr179KDTree::xr_new();
        tree.xr_insert(super::Xr179KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_179_kdtree_insert_multiple() {
        let mut tree = super::Xr179KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr179KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_179_kdtree_nearest_neighbor() {
        let mut tree = super::Xr179KDTree::xr_new();
        tree.xr_insert(super::Xr179KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr179KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr179KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_179_kdtree_nn_empty() {
        let tree = super::Xr179KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr179KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_179_kdtree_range_search() {
        let mut tree = super::Xr179KDTree::xr_new();
        tree.xr_insert(super::Xr179KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr179KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr179KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_179_kdtree_range_empty() {
        let mut tree = super::Xr179KDTree::xr_new();
        tree.xr_insert(super::Xr179KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_179_kdtree_all_points() {
        let mut tree = super::Xr179KDTree::xr_new();
        tree.xr_insert(super::Xr179KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr179KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_179_kdtree_depth() {
        let mut tree = super::Xr179KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr179KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_179_kdtree_bounding_box() {
        let mut tree = super::Xr179KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr179KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr179KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
