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

}
