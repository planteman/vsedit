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
}
