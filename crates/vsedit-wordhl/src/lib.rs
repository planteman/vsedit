//! Word highlight (same symbol highlighting).
//!
//! Finds and highlights all occurrences of the word under the cursor,
//! distinguishing between read and write references.

use std::collections::HashMap;
use std::fmt;
/// Kind of highlight for a document symbol occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentHighlightKind {
    Text,
    Read,
    Write,
}

impl DocumentHighlightKind {
    /// Human-readable label for this kind.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

impl std::fmt::Display for DocumentHighlightKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Convenience alias.
pub type HighlightKind = DocumentHighlightKind;

/// A highlight range within a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub kind: DocumentHighlightKind,
}

impl DocumentHighlight {
    pub fn new(line: u32, start_column: u32, end_column: u32, kind: DocumentHighlightKind) -> Self {
        Self { line, start_column, end_column, kind }
    }

    /// Length of the highlight in columns.
    pub fn len(&self) -> u32 {
        self.end_column.saturating_sub(self.start_column)
    }

    /// Width of the highlight span in columns.
    pub fn span(&self) -> u32 {
        self.end_column.saturating_sub(self.start_column)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns true if this highlight is on the given line.
    pub fn is_on_line(&self, line: u32) -> bool {
        self.line == line
    }

    /// Returns true if the given column falls within this highlight range.
    pub fn contains_column(&self, column: u32) -> bool {
        column >= self.start_column && column < self.end_column
    }

    /// Returns true if two highlights overlap on the same line.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.line == other.line
            && self.start_column < other.end_column
            && other.start_column < self.end_column
    }

    /// Change the kind of this highlight, returning a new instance.
    pub fn with_kind(mut self, kind: DocumentHighlightKind) -> Self {
        self.kind = kind;
        self
    }
}

impl std::fmt::Display for DocumentHighlight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}..{} ({})",
            self.line, self.start_column, self.end_column, self.kind
        )
    }
}

/// Extract the word at a given position (1-based line and column).
///
/// Returns `(word, start_column, end_column)` or `None` if no word is found.
pub fn find_word_at_position(lines: &[&str], line: u32, column: u32) -> Option<(String, u32, u32)> {
    if line == 0 || line as usize > lines.len() {
        return None;
    }
    let text = lines[(line - 1) as usize];
    if column == 0 || column as usize > text.len() {
        return None;
    }
    let col_idx = (column - 1) as usize;
    let bytes = text.as_bytes();

    if !is_word_char(bytes[col_idx]) {
        return None;
    }

    let mut start = col_idx;
    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = col_idx;
    while end < bytes.len() && is_word_char(bytes[end]) {
        end += 1;
    }

    let word = text[start..end].to_string();
    Some((word, (start + 1) as u32, (end + 1) as u32))
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Find all occurrences of a word in text (whole-word matching).
pub fn find_word_highlights(lines: &[&str], word: &str) -> Vec<DocumentHighlight> {
    let mut highlights = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut start = 0;
        while let Some(pos) = line[start..].find(word) {
            let abs_pos = start + pos;
            let before_ok = abs_pos == 0 || !is_word_char(line.as_bytes()[abs_pos - 1]);
            let after_pos = abs_pos + word.len();
            let after_ok = after_pos >= line.len() || !is_word_char(line.as_bytes()[after_pos]);
            if before_ok && after_ok {
                highlights.push(DocumentHighlight {
                    line: (i + 1) as u32,
                    start_column: (abs_pos + 1) as u32,
                    end_column: (after_pos + 1) as u32,
                    kind: DocumentHighlightKind::Text,
                });
            }
            start = abs_pos + word.len();
        }
    }
    highlights
}

/// Provider trait for document highlights.
pub trait DocumentHighlightProvider: Send + Sync {
    fn provide_document_highlights(&self, uri: &str, line: u32, column: u32) -> Vec<DocumentHighlight>;
}

/// Alias for the provider trait.
pub trait WordHighlightProvider: Send + Sync {
    fn highlight(&self, uri: &str, line: u32, column: u32) -> Vec<DocumentHighlight>;
}

/// Errors that can occur during highlight operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordHighlightError {
    /// The requested line is out of range.
    LineOutOfRange { line: u32, max_line: u32 },
    /// The requested column is out of range.
    ColumnOutOfRange { column: u32, line_length: u32 },
    /// The position does not point to a word character.
    NotAWordChar { line: u32, column: u32 },
    /// An empty word was supplied.
    EmptyWord,
}

impl std::fmt::Display for WordHighlightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LineOutOfRange { line, max_line } => {
                write!(f, "line {} is out of range (max {})", line, max_line)
            }
            Self::ColumnOutOfRange { column, line_length } => {
                write!(f, "column {} is out of range (line length {})", column, line_length)
            }
            Self::NotAWordChar { line, column } => {
                write!(f, "position {}:{} is not a word character", line, column)
            }
            Self::EmptyWord => write!(f, "empty word"),
        }
    }
}

impl std::error::Error for WordHighlightError {}

/// Find the word at a position, returning a detailed error on failure.
pub fn try_find_word_at_position(
    lines: &[&str],
    line: u32,
    column: u32,
) -> Result<(String, u32, u32), WordHighlightError> {
    if line == 0 || line as usize > lines.len() {
        return Err(WordHighlightError::LineOutOfRange {
            line,
            max_line: lines.len() as u32,
        });
    }
    let text = lines[(line - 1) as usize];
    if column == 0 || column as usize > text.len() {
        return Err(WordHighlightError::ColumnOutOfRange {
            column,
            line_length: text.len() as u32,
        });
    }
    let col_idx = (column - 1) as usize;
    let bytes = text.as_bytes();
    if !is_word_char(bytes[col_idx]) {
        return Err(WordHighlightError::NotAWordChar { line, column });
    }

    let mut start = col_idx;
    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = col_idx;
    while end < bytes.len() && is_word_char(bytes[end]) {
        end += 1;
    }
    let word = text[start..end].to_string();
    Ok((word, (start + 1) as u32, (end + 1) as u32))
}

/// A collection of highlights with helper methods.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HighlightSet {
    highlights: Vec<DocumentHighlight>,
}

impl HighlightSet {
    pub fn new() -> Self {
        Self { highlights: Vec::new() }
    }

    pub fn from_highlights(highlights: Vec<DocumentHighlight>) -> Self {
        Self { highlights }
    }

    pub fn push(&mut self, highlight: DocumentHighlight) {
        self.highlights.push(highlight);
    }

    pub fn len(&self) -> usize {
        self.highlights.len()
    }

    pub fn is_empty(&self) -> bool {
        self.highlights.is_empty()
    }

    /// Return all highlights on a specific line.
    pub fn on_line(&self, line: u32) -> Vec<&DocumentHighlight> {
        self.highlights.iter().filter(|h| h.line == line).collect()
    }

    /// Return highlights filtered by kind.
    pub fn by_kind(&self, kind: DocumentHighlightKind) -> Vec<&DocumentHighlight> {
        self.highlights.iter().filter(|h| h.kind == kind).collect()
    }

    /// Count of distinct lines that contain at least one highlight.
    pub fn distinct_line_count(&self) -> usize {
        let mut lines: Vec<u32> = self.highlights.iter().map(|h| h.line).collect();
        lines.sort_unstable();
        lines.dedup();
        lines.len()
    }

    /// Return highlights sorted by position (line, then start_column).
    pub fn sorted(&self) -> Vec<&DocumentHighlight> {
        let mut refs: Vec<&DocumentHighlight> = self.highlights.iter().collect();
        refs.sort_by_key(|h| (h.line, h.start_column));
        refs
    }

    pub fn iter(&self) -> std::slice::Iter<'_, DocumentHighlight> {
        self.highlights.iter()
    }

    pub fn into_inner(self) -> Vec<DocumentHighlight> {
        self.highlights
    }

    /// Merge all highlights from `other` into this set.
    pub fn merge(&mut self, other: HighlightSet) {
        self.highlights.extend(other.highlights);
    }

    /// Remove all highlights on the given line. Returns the number removed.
    pub fn remove_on_line(&mut self, line: u32) -> usize {
        let before = self.highlights.len();
        self.highlights.retain(|h| h.line != line);
        before - self.highlights.len()
    }

    /// Remove all highlights.
    pub fn clear(&mut self) {
        self.highlights.clear();
    }

    /// Return sorted unique line numbers that contain at least one highlight.
    pub fn lines(&self) -> Vec<u32> {
        let mut lines: Vec<u32> = self.highlights.iter().map(|h| h.line).collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Check whether any highlight in the set contains the given position.
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        self.highlights
            .iter()
            .any(|h| h.line == line && h.contains_column(col))
    }
}

impl IntoIterator for HighlightSet {
    type Item = DocumentHighlight;
    type IntoIter = std::vec::IntoIter<DocumentHighlight>;
    fn into_iter(self) -> Self::IntoIter {
        self.highlights.into_iter()
    }
}

impl std::fmt::Display for HighlightSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} highlights on {} lines",
            self.highlights.len(),
            self.distinct_line_count()
        )
    }
}

/// Service that manages word highlight providers and resolves highlights.
pub struct WordHighlightService {
    providers: Vec<Box<dyn WordHighlightProvider>>,
}

impl WordHighlightService {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Box<dyn WordHighlightProvider>) {
        self.providers.push(provider);
    }

    /// Get highlights from all providers.
    pub fn highlights(&self, uri: &str, line: u32, column: u32) -> Vec<DocumentHighlight> {
        let mut all = Vec::new();
        for provider in &self.providers {
            all.extend(provider.highlight(uri, line, column));
        }
        all
    }

    /// Fallback: find word at position and highlight all occurrences in the given text.
    pub fn highlight_word_occurrences(
        lines: &[&str],
        cursor_line: u32,
        cursor_column: u32,
    ) -> Vec<DocumentHighlight> {
        match find_word_at_position(lines, cursor_line, cursor_column) {
            Some((word, _, _)) => find_word_highlights(lines, &word),
            None => Vec::new(),
        }
    }
}

impl Default for WordHighlightService {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics computed from a set of highlights.
#[derive(Debug, Clone, PartialEq)]
pub struct HighlightStats {
    /// Total number of highlight ranges.
    pub total_highlights: usize,
    /// Number of unique highlighted words.
    pub unique_words: usize,
    /// Average number of occurrences per unique word.
    pub avg_occurrences: f64,
    /// The word that appears most frequently, if any.
    pub most_highlighted_word: Option<String>,
}

/// Compute statistics from highlights and the source text they refer to.
///
/// Each highlight's word is extracted from `lines` using its positional data.
pub fn compute_highlight_stats(
    highlights: &[DocumentHighlight],
    lines: &[&str],
) -> HighlightStats {
    if highlights.is_empty() {
        return HighlightStats {
            total_highlights: 0,
            unique_words: 0,
            avg_occurrences: 0.0,
            most_highlighted_word: None,
        };
    }

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for h in highlights {
        let line_idx = h.line.saturating_sub(1) as usize;
        if line_idx < lines.len() {
            let start = h.start_column.saturating_sub(1) as usize;
            let end = h.end_column.saturating_sub(1) as usize;
            let text = lines[line_idx];
            if end <= text.len() && start < end {
                let word = &text[start..end];
                *counts.entry(word.to_string()).or_insert(0) += 1;
            }
        }
    }

    let unique_words = counts.len();
    let total_highlights = highlights.len();
    let avg_occurrences = if unique_words > 0 {
        total_highlights as f64 / unique_words as f64
    } else {
        0.0
    };
    let most_highlighted_word = counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(word, _)| word);

    HighlightStats {
        total_highlights,
        unique_words,
        avg_occurrences,
        most_highlighted_word,
    }
}

/// A word boundary within a line of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordBoundary {
    /// Byte offset where the word starts (0-based).
    pub start: usize,
    /// Byte offset one past the last character of the word (0-based).
    pub end: usize,
    /// The word text.
    pub word: String,
}

/// Find all word boundaries in a single line of text.
///
/// Words consist of ASCII alphanumeric characters and underscores.
pub fn find_word_boundaries(text: &str) -> Vec<WordBoundary> {
    let bytes = text.as_bytes();
    let mut boundaries = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if is_word_char(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_word_char(bytes[i]) {
                i += 1;
            }
            boundaries.push(WordBoundary {
                start,
                end: i,
                word: text[start..i].to_string(),
            });
        } else {
            i += 1;
        }
    }
    boundaries
}

/// Merge overlapping or adjacent highlight ranges on the same line.
///
/// When ranges overlap the merged range uses the kind of the first range
/// encountered (by start column).  Ranges on different lines are never merged.
pub fn highlight_merge(highlights: &[DocumentHighlight]) -> Vec<DocumentHighlight> {
    if highlights.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<DocumentHighlight> = highlights.to_vec();
    sorted.sort_by_key(|h| (h.line, h.start_column));

    let mut merged: Vec<DocumentHighlight> = Vec::new();
    merged.push(sorted[0].clone());

    for h in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        if h.line == last.line && h.start_column <= last.end_column {
            // Overlapping or adjacent on the same line — extend.
            if h.end_column > last.end_column {
                last.end_column = h.end_column;
            }
        } else {
            merged.push(h.clone());
        }
    }
    merged
}

// ---------------------------------------------------------------------------
// Debounce — cursor-move debouncing for auto-update
// ---------------------------------------------------------------------------

/// Tracks cursor moves and determines when to re-compute highlights.
#[derive(Debug, Clone)]
pub struct HighlightDebounce {
    /// Debounce interval in milliseconds.
    pub delay_ms: u64,
    /// Last cursor position that triggered a highlight update.
    last_line: u32,
    last_column: u32,
    /// Timestamp (in ms) of the last cursor move.
    last_move_ms: u64,
    /// Whether we have a pending (un-fired) update.
    pending: bool,
}

impl HighlightDebounce {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            last_line: 0,
            last_column: 0,
            last_move_ms: 0,
            pending: false,
        }
    }

    /// Record a cursor move. Returns `true` if the position actually changed.
    pub fn cursor_moved(&mut self, line: u32, column: u32, now_ms: u64) -> bool {
        if line == self.last_line && column == self.last_column {
            return false;
        }
        self.last_line = line;
        self.last_column = column;
        self.last_move_ms = now_ms;
        self.pending = true;
        true
    }

    /// Check whether enough time has elapsed since the last move to fire.
    /// Returns `true` at most once per debounce window; clears the pending
    /// flag when it fires.
    pub fn should_update(&mut self, now_ms: u64) -> bool {
        if !self.pending {
            return false;
        }
        if now_ms.saturating_sub(self.last_move_ms) >= self.delay_ms {
            self.pending = false;
            true
        } else {
            false
        }
    }

    /// Current pending cursor position.
    pub fn position(&self) -> (u32, u32) {
        (self.last_line, self.last_column)
    }

    /// Whether an update is pending.
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// Cancel any pending update.
    pub fn cancel(&mut self) {
        self.pending = false;
    }
}

impl Default for HighlightDebounce {
    fn default() -> Self {
        Self::new(150) // 150ms default debounce
    }
}

/// Configuration for document highlight rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightRenderConfig {
    /// Background color index for read highlights (256-color terminal).
    pub read_bg: u8,
    /// Background color index for write highlights.
    pub write_bg: u8,
    /// Background color index for text highlights.
    pub text_bg: u8,
    /// Debounce delay in milliseconds.
    pub debounce_ms: u64,
}

impl Default for HighlightRenderConfig {
    fn default() -> Self {
        Self {
            read_bg: 238,  // dark gray
            write_bg: 52,  // dark red
            text_bg: 236,  // very dark gray
            debounce_ms: 150,
        }
    }
}

impl HighlightRenderConfig {
    /// Return the background color index for a given highlight kind.
    pub fn bg_for_kind(&self, kind: DocumentHighlightKind) -> u8 {
        match kind {
            DocumentHighlightKind::Read => self.read_bg,
            DocumentHighlightKind::Write => self.write_bg,
            DocumentHighlightKind::Text => self.text_bg,
        }
    }
}

/// Category of a symbol for highlight classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolCategory {
    Variable,
    Function,
    Type,
    Keyword,
    Literal,
    Unknown,
}

impl std::fmt::Display for SymbolCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Variable => write!(f, "variable"),
            Self::Function => write!(f, "function"),
            Self::Type => write!(f, "type"),
            Self::Keyword => write!(f, "keyword"),
            Self::Literal => write!(f, "literal"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Categorize a word based on simple heuristics.
pub fn categorize_symbol(word: &str) -> SymbolCategory {
    const KEYWORDS: &[&str] = &[
        "fn", "let", "mut", "if", "else", "match", "for", "while", "loop",
        "return", "struct", "enum", "impl", "pub", "use", "mod", "const",
        "static", "trait", "type", "where", "async", "await", "self", "super",
    ];
    if KEYWORDS.contains(&word) {
        return SymbolCategory::Keyword;
    }
    if word.chars().next().map_or(false, |c| c.is_uppercase()) {
        return SymbolCategory::Type;
    }
    if word.chars().all(|c| c.is_ascii_digit() || c == '_') && !word.is_empty() {
        return SymbolCategory::Literal;
    }
    SymbolCategory::Variable
}

/// Priority level for highlight rendering order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HighlightPriority(pub u32);

impl HighlightPriority {
    pub const LOW: Self = Self(0);
    pub const NORMAL: Self = Self(50);
    pub const HIGH: Self = Self(100);
}

/// A highlight with an associated priority for rendering order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrioritizedHighlight {
    pub highlight: DocumentHighlight,
    pub priority: HighlightPriority,
}

/// Sort highlights by priority (highest first), breaking ties by position.
pub fn sort_by_priority(highlights: &mut [PrioritizedHighlight]) {
    highlights.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.highlight.line.cmp(&b.highlight.line))
            .then_with(|| a.highlight.start_column.cmp(&b.highlight.start_column))
    });
}

/// Tracks highlights for multiple words simultaneously.
#[derive(Debug, Clone, Default)]
pub struct MultiWordTracker {
    entries: Vec<(String, HighlightSet)>,
}

impl MultiWordTracker {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Add or replace highlights for a word.
    pub fn set_word(&mut self, word: String, highlights: HighlightSet) {
        if let Some(entry) = self.entries.iter_mut().find(|(w, _)| *w == word) {
            entry.1 = highlights;
        } else {
            self.entries.push((word, highlights));
        }
    }

    /// Remove tracking for a word. Returns true if found.
    pub fn remove_word(&mut self, word: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(w, _)| w != word);
        self.entries.len() < before
    }

    /// Get highlights for a specific word.
    pub fn get_word(&self, word: &str) -> Option<&HighlightSet> {
        self.entries.iter().find(|(w, _)| w == word).map(|(_, hs)| hs)
    }

    /// Return the number of tracked words.
    pub fn word_count(&self) -> usize {
        self.entries.len()
    }

    /// Return total highlight count across all words.
    pub fn total_highlights(&self) -> usize {
        self.entries.iter().map(|(_, hs)| hs.len()).sum()
    }

    /// Clear all tracked words.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// HighlightMerger — merge overlapping highlights on the same line
// ---------------------------------------------------------------------------

/// Merges overlapping or adjacent highlights on the same line.
///
/// When multiple highlights overlap, they are combined into one span
/// covering the full extent.  The resulting kind is chosen by priority:
/// `Write > Read > Text`.
#[derive(Debug, Clone)]
pub struct HighlightMerger {
    highlights: Vec<DocumentHighlight>,
}

impl HighlightMerger {
    /// Create a new empty merger.
    pub fn new() -> Self {
        Self {
            highlights: Vec::new(),
        }
    }

    /// Create a merger pre-loaded with highlights.
    pub fn from_highlights(highlights: Vec<DocumentHighlight>) -> Self {
        Self { highlights }
    }

    /// Add a highlight to the pending set.
    pub fn push(&mut self, hl: DocumentHighlight) {
        self.highlights.push(hl);
    }

    /// Merge all overlapping highlights on the same line and return the
    /// resulting non-overlapping set.
    pub fn merge(&self) -> Vec<DocumentHighlight> {
        if self.highlights.is_empty() {
            return Vec::new();
        }

        let mut by_line: std::collections::HashMap<u32, Vec<&DocumentHighlight>> =
            std::collections::HashMap::new();
        for hl in &self.highlights {
            by_line.entry(hl.line).or_default().push(hl);
        }

        let mut result = Vec::new();
        for (_line, mut hls) in by_line {
            hls.sort_by_key(|h| (h.start_column, h.end_column));
            let mut cur_start = hls[0].start_column;
            let mut cur_end = hls[0].end_column;
            let mut cur_kind = hls[0].kind;
            let cur_line = hls[0].line;

            for hl in hls.iter().skip(1) {
                if hl.start_column <= cur_end {
                    // overlapping — extend and pick higher-priority kind
                    cur_end = cur_end.max(hl.end_column);
                    cur_kind = higher_priority_kind(cur_kind, hl.kind);
                } else {
                    result.push(DocumentHighlight::new(cur_line, cur_start, cur_end, cur_kind));
                    cur_start = hl.start_column;
                    cur_end = hl.end_column;
                    cur_kind = hl.kind;
                }
            }
            result.push(DocumentHighlight::new(cur_line, cur_start, cur_end, cur_kind));
        }
        result.sort_by_key(|h| (h.line, h.start_column));
        result
    }
}

/// Returns the higher-priority kind (Write > Read > Text).
fn higher_priority_kind(a: DocumentHighlightKind, b: DocumentHighlightKind) -> DocumentHighlightKind {
    fn rank(k: DocumentHighlightKind) -> u8 {
        match k {
            DocumentHighlightKind::Text => 0,
            DocumentHighlightKind::Read => 1,
            DocumentHighlightKind::Write => 2,
        }
    }
    if rank(b) > rank(a) { b } else { a }
}

// ---------------------------------------------------------------------------
// Highlight diff — compute added/removed highlights between two sets
// ---------------------------------------------------------------------------

/// Result of diffing two highlight sets.
#[derive(Debug, Clone)]
pub struct HighlightDiff {
    /// Highlights present in `after` but not in `before`.
    pub added: Vec<DocumentHighlight>,
    /// Highlights present in `before` but not in `after`.
    pub removed: Vec<DocumentHighlight>,
}

impl HighlightDiff {
    /// Returns `true` if there are no differences.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Total number of changes (additions + removals).
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len()
    }
}

/// Compute the diff between two highlight sets.
///
/// Two highlights are considered equal when they have the same line,
/// start_column, end_column, and kind.
pub fn diff_highlights(before: &[DocumentHighlight], after: &[DocumentHighlight]) -> HighlightDiff {
    fn key(h: &DocumentHighlight) -> (u32, u32, u32, DocumentHighlightKind) {
        (h.line, h.start_column, h.end_column, h.kind)
    }

    let before_set: std::collections::HashSet<_> = before.iter().map(key).collect();
    let after_set: std::collections::HashSet<_> = after.iter().map(key).collect();

    let added = after
        .iter()
        .filter(|h| !before_set.contains(&key(h)))
        .cloned()
        .collect();
    let removed = before
        .iter()
        .filter(|h| !after_set.contains(&key(h)))
        .cloned()
        .collect();

    HighlightDiff { added, removed }
}

// ---------------------------------------------------------------------------
// WordOccurrenceTracker — count word occurrences with positions
// ---------------------------------------------------------------------------

/// Tracks a single occurrence of a word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordOccurrence {
    /// Zero-based line index.
    pub line: u32,
    /// Zero-based column of the start of the word.
    pub column: u32,
    /// Length of the word in columns.
    pub length: u32,
}

/// Counts and tracks all occurrences of specific words in a document.
#[derive(Debug, Clone)]
pub struct WordOccurrenceTracker {
    occurrences: std::collections::HashMap<String, Vec<WordOccurrence>>,
}

impl WordOccurrenceTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            occurrences: std::collections::HashMap::new(),
        }
    }

    /// Scan `lines` for all occurrences of `word` (case-sensitive).
    pub fn track_word(&mut self, lines: &[&str], word: &str) {
        let mut positions = Vec::new();
        for (line_idx, line) in lines.iter().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(word) {
                let col = start + pos;
                // only match whole words
                let before_ok = col == 0
                    || !line.as_bytes()[col - 1].is_ascii_alphanumeric()
                        && line.as_bytes()[col - 1] != b'_';
                let after_pos = col + word.len();
                let after_ok = after_pos >= line.len()
                    || !line.as_bytes()[after_pos].is_ascii_alphanumeric()
                        && line.as_bytes()[after_pos] != b'_';

                if before_ok && after_ok {
                    positions.push(WordOccurrence {
                        line: line_idx as u32,
                        column: col as u32,
                        length: word.len() as u32,
                    });
                }
                start = col + word.len();
            }
        }
        self.occurrences.insert(word.to_string(), positions);
    }

    /// Return the number of occurrences of `word`.
    pub fn count(&self, word: &str) -> usize {
        self.occurrences.get(word).map_or(0, |v| v.len())
    }

    /// Return all tracked occurrences of `word`.
    pub fn get(&self, word: &str) -> &[WordOccurrence] {
        self.occurrences.get(word).map_or(&[], |v| v.as_slice())
    }

    /// Return all tracked words.
    pub fn tracked_words(&self) -> Vec<&str> {
        self.occurrences.keys().map(|s| s.as_str()).collect()
    }

    /// Remove tracking data for a word.
    pub fn untrack(&mut self, word: &str) {
        self.occurrences.remove(word);
    }

    /// Clear all tracking data.
    pub fn clear(&mut self) {
        self.occurrences.clear();
    }

    /// Return the total number of occurrences across all tracked words.
    pub fn total_occurrences(&self) -> usize {
        self.occurrences.values().map(|v| v.len()).sum()
    }

    /// Return lines that contain at least one occurrence of `word`.
    pub fn lines_with_word(&self, word: &str) -> Vec<u32> {
        let mut lines: Vec<u32> = self
            .get(word)
            .iter()
            .map(|o| o.line)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        lines.sort();
        lines
    }
}

// ---------------------------------------------------------------------------
// Highlight filtering and analysis utilities
// ---------------------------------------------------------------------------

/// Filter highlights to only those on a given set of lines.
pub fn filter_highlights_on_lines(highlights: &[DocumentHighlight], lines: &[u32]) -> Vec<DocumentHighlight> {
    highlights
        .iter()
        .filter(|h| lines.contains(&h.line))
        .cloned()
        .collect()
}

/// Count how many highlights of each kind are present.
pub fn count_by_kind(highlights: &[DocumentHighlight]) -> (usize, usize, usize) {
    let mut text = 0;
    let mut read = 0;
    let mut write = 0;
    for h in highlights {
        match h.kind {
            DocumentHighlightKind::Text => text += 1,
            DocumentHighlightKind::Read => read += 1,
            DocumentHighlightKind::Write => write += 1,
        }
    }
    (text, read, write)
}

/// Compute the total character span covered by all highlights.
pub fn total_highlight_span(highlights: &[DocumentHighlight]) -> u32 {
    highlights.iter().map(|h| h.span()).sum()
}

/// Find all highlights that contain a given column on a given line.
pub fn highlights_at_position(highlights: &[DocumentHighlight], line: u32, column: u32) -> Vec<&DocumentHighlight> {
    highlights
        .iter()
        .filter(|h| h.is_on_line(line) && h.contains_column(column))
        .collect()
}

/// Check if any two highlights in the set overlap.
pub fn has_overlapping_highlights(highlights: &[DocumentHighlight]) -> bool {
    for i in 0..highlights.len() {
        for j in (i + 1)..highlights.len() {
            if highlights[i].overlaps(&highlights[j]) {
                return true;
            }
        }
    }
    false
}

/// Return the line numbers that have at least one write highlight.
pub fn lines_with_writes(highlights: &[DocumentHighlight]) -> Vec<u32> {
    let mut lines: Vec<u32> = highlights
        .iter()
        .filter(|h| matches!(h.kind, DocumentHighlightKind::Write))
        .map(|h| h.line)
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

// ---------------------------------------------------------------------------
// Highlight filtering and statistics
// ---------------------------------------------------------------------------

/// Return only highlights of a specific kind.
pub fn filter_by_kind(
    highlights: &[DocumentHighlight],
    kind: DocumentHighlightKind,
) -> Vec<&DocumentHighlight> {
    highlights.iter().filter(|h| h.kind == kind).collect()
}

/// Return highlights that overlap a given column range on a specific line.
pub fn highlights_at(
    highlights: &[DocumentHighlight],
    line: u32,
    column: u32,
) -> Vec<&DocumentHighlight> {
    highlights
        .iter()
        .filter(|h| h.is_on_line(line) && h.contains_column(column))
        .collect()
}

/// Sort highlights by position (line, then start_column).
pub fn sort_highlights(highlights: &mut [DocumentHighlight]) {
    highlights.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then(a.start_column.cmp(&b.start_column))
    });
}

/// Remove highlights that fully overlap with a wider highlight on the same line.
pub fn remove_subsumed_highlights(highlights: &[DocumentHighlight]) -> Vec<DocumentHighlight> {
    let mut result = Vec::new();
    for (i, h) in highlights.iter().enumerate() {
        let subsumed = highlights.iter().enumerate().any(|(j, other)| {
            i != j
                && other.line == h.line
                && other.start_column <= h.start_column
                && other.end_column >= h.end_column
                && (other.start_column < h.start_column || other.end_column > h.end_column)
        });
        if !subsumed {
            result.push(h.clone());
        }
    }
    result
}

/// Return the total number of unique lines that have at least one highlight.
pub fn highlighted_line_count(highlights: &[DocumentHighlight]) -> usize {
    let mut lines: Vec<u32> = highlights.iter().map(|h| h.line).collect();
    lines.sort();
    lines.dedup();
    lines.len()
}

/// Return the highlight with the widest span (longest range).
pub fn widest_highlight(highlights: &[DocumentHighlight]) -> Option<&DocumentHighlight> {
    highlights.iter().max_by_key(|h| h.span())
}

/// Build a summary string like "3 text, 1 read, 2 write".
pub fn highlight_summary(highlights: &[DocumentHighlight]) -> String {
    let (text, read, write) = count_by_kind(highlights);
    format!("{text} text, {read} read, {write} write")
}

/// Create a highlight that spans an entire line of text.
pub fn highlight_full_line(line: u32, line_text: &str, kind: DocumentHighlightKind) -> DocumentHighlight {
    DocumentHighlight::new(line, 1, (line_text.len() + 1) as u32, kind)
}

/// Navigator for cycling through a list of highlights.
pub struct WordHighlightNavigation {
    pub highlights: Vec<DocumentHighlight>,
    pub current_index: Option<usize>,
}

impl WordHighlightNavigation {
    pub fn new(highlights: Vec<DocumentHighlight>) -> Self {
        let current_index = if highlights.is_empty() { None } else { Some(0) };
        Self { highlights, current_index }
    }

    /// Advance to the next highlight, wrapping around to the beginning.
    pub fn next(&mut self) -> Option<&DocumentHighlight> {
        if self.highlights.is_empty() {
            return None;
        }
        let idx = match self.current_index {
            Some(i) => (i + 1) % self.highlights.len(),
            None => 0,
        };
        self.current_index = Some(idx);
        Some(&self.highlights[idx])
    }

    /// Go to the previous highlight, wrapping around to the end.
    pub fn previous(&mut self) -> Option<&DocumentHighlight> {
        if self.highlights.is_empty() {
            return None;
        }
        let idx = match self.current_index {
            Some(0) => self.highlights.len() - 1,
            Some(i) => i - 1,
            None => self.highlights.len() - 1,
        };
        self.current_index = Some(idx);
        Some(&self.highlights[idx])
    }

    /// Return the current highlight without advancing.
    pub fn current(&self) -> Option<&DocumentHighlight> {
        self.current_index.map(|i| &self.highlights[i])
    }

    /// Return the number of highlights.
    pub fn count(&self) -> usize {
        self.highlights.len()
    }

    /// Return a label like "3 of 5" describing the current position.
    pub fn position_label(&self) -> String {
        match self.current_index {
            Some(i) => format!("{} of {}", i + 1, self.highlights.len()),
            None => String::from("0 of 0"),
        }
    }

    /// Reset the navigation back to the first highlight.
    pub fn reset(&mut self) {
        self.current_index = if self.highlights.is_empty() { None } else { Some(0) };
    }
}

/// Scope that limits highlight searching to a range of lines.
pub struct WordHighlightScope {
    pub start_line: u32,
    pub end_line: u32,
}

impl WordHighlightScope {
    pub fn new(start: u32, end: u32) -> Self {
        Self {
            start_line: start.min(end),
            end_line: start.max(end),
        }
    }

    /// Check whether a given line number falls within this scope.
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// Filter a slice of highlights to only those within this scope.
    pub fn filter(&self, highlights: &[DocumentHighlight]) -> Vec<DocumentHighlight> {
        highlights
            .iter()
            .filter(|h| self.contains_line(h.line))
            .cloned()
            .collect()
    }

    /// Return how many lines this scope covers (inclusive).
    pub fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Expand the scope by `lines` in each direction (saturating at 0).
    pub fn expand(&mut self, lines: u32) {
        self.start_line = self.start_line.saturating_sub(lines);
        self.end_line = self.end_line.saturating_add(lines);
    }

    /// Shrink the scope by `lines` in each direction, keeping at least 1 line.
    pub fn shrink(&mut self, lines: u32) {
        let mid = self.start_line / 2 + self.end_line / 2;
        self.start_line = self.start_line.saturating_add(lines).min(mid);
        self.end_line = if self.end_line.saturating_sub(lines) < self.start_line {
            self.start_line
        } else {
            self.end_line.saturating_sub(lines)
        };
    }
}

/// Highlight provider with semantic awareness (case sensitivity, whole-word, category filtering).
pub struct SemanticHighlightProvider {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub symbol_categories: Vec<SymbolCategory>,
}

impl SemanticHighlightProvider {
    pub fn new() -> Self {
        Self {
            case_sensitive: true,
            whole_word: true,
            symbol_categories: Vec::new(),
        }
    }

    pub fn with_case_sensitive(mut self, v: bool) -> Self {
        self.case_sensitive = v;
        self
    }

    pub fn with_whole_word(mut self, v: bool) -> Self {
        self.whole_word = v;
        self
    }

    pub fn with_categories(mut self, cats: Vec<SymbolCategory>) -> Self {
        self.symbol_categories = cats;
        self
    }

    /// Find highlights in the given lines, respecting case sensitivity and whole-word settings.
    pub fn find_highlights(&self, lines: &[&str], word: &str) -> Vec<DocumentHighlight> {
        let search_word: String;
        let needle: &str;
        if self.case_sensitive {
            needle = word;
        } else {
            search_word = word.to_lowercase();
            needle = &search_word;
        }

        let mut highlights = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let haystack: String;
            let hay: &str;
            if self.case_sensitive {
                hay = line;
            } else {
                haystack = line.to_lowercase();
                hay = &haystack;
            }

            let mut start = 0;
            while let Some(pos) = hay[start..].find(needle) {
                let abs_pos = start + pos;
                let after_pos = abs_pos + needle.len();

                let accept = if self.whole_word {
                    let before_ok =
                        abs_pos == 0 || !is_word_char(line.as_bytes()[abs_pos - 1]);
                    let after_ok =
                        after_pos >= line.len() || !is_word_char(line.as_bytes()[after_pos]);
                    before_ok && after_ok
                } else {
                    true
                };

                if accept {
                    highlights.push(DocumentHighlight {
                        line: (i + 1) as u32,
                        start_column: (abs_pos + 1) as u32,
                        end_column: (after_pos + 1) as u32,
                        kind: DocumentHighlightKind::Text,
                    });
                }
                start = abs_pos + needle.len();
            }
        }
        highlights
    }

    /// Check whether the word's category matches one of the configured categories.
    pub fn matches_category(&self, word: &str) -> bool {
        if self.symbol_categories.is_empty() {
            return true;
        }
        let cat = categorize_symbol(word);
        self.symbol_categories.contains(&cat)
    }
}

/// Manages throttling of highlight requests to avoid excessive recomputation.
pub struct HighlightThrottler {
    pub last_request_ms: u64,
    pub interval_ms: u64,
    pub pending: Option<String>,
}

impl HighlightThrottler {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            last_request_ms: 0,
            interval_ms,
            pending: None,
        }
    }

    /// Check whether enough time has elapsed to process a new request.
    pub fn should_process(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_request_ms) >= self.interval_ms
    }

    /// Submit a request. Returns `true` if it should be processed immediately.
    pub fn request(&mut self, word: String, now_ms: u64) -> bool {
        if self.should_process(now_ms) {
            self.last_request_ms = now_ms;
            self.pending = None;
            true
        } else {
            self.pending = Some(word);
            false
        }
    }

    /// Take the pending word, if any, clearing it.
    pub fn take_pending(&mut self) -> Option<String> {
        self.pending.take()
    }

    /// Clear all pending state.
    pub fn clear(&mut self) {
        self.pending = None;
    }
}


// === Word Highlight Semantic Filter ===

/// Word Highlight Semantic Filter implementation.
#[derive(Debug, Clone)]
pub struct WordHighlightSemanticFilter {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: WordHighlightSemanticFilterStats,
}

/// Statistics for WordHighlightSemanticFilter.
#[derive(Debug, Clone, Default)]
pub struct WordHighlightSemanticFilterStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl WordHighlightSemanticFilterStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl WordHighlightSemanticFilter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: WordHighlightSemanticFilterStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &WordHighlightSemanticFilterStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for WordHighlightSemanticFilter {
    fn default() -> Self {
        Self::new()
    }
}

// === Word Highlight Animation ===

/// Priority level for WordHighlightAnimation items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WordHighlightAnimationPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl WordHighlightAnimationPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for WordHighlightAnimationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Word Highlight Animation implementation.
#[derive(Debug, Clone)]
pub struct WordHighlightAnimation {
    items: Vec<WordHighlightAnimationItem>,
    max_items: usize,
    default_priority: WordHighlightAnimationPriority,
}

/// A single item in WordHighlightAnimation.
#[derive(Debug, Clone)]
pub struct WordHighlightAnimationItem {
    pub id: String,
    pub label: String,
    pub priority: WordHighlightAnimationPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl WordHighlightAnimationItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: WordHighlightAnimationPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: WordHighlightAnimationPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl WordHighlightAnimation {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: WordHighlightAnimationPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: WordHighlightAnimationItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<WordHighlightAnimationItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&WordHighlightAnimationItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: WordHighlightAnimationPriority) -> Vec<&WordHighlightAnimationItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&WordHighlightAnimationItem> {
        let mut sorted: Vec<&WordHighlightAnimationItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&WordHighlightAnimationItem> {
        let mut sorted: Vec<&WordHighlightAnimationItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&WordHighlightAnimationItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: WordHighlightAnimationPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> WordHighlightAnimationPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &WordHighlightAnimationItem> {
        self.items.iter()
    }
}

impl Default for WordHighlightAnimation {
    fn default() -> Self {
        Self::new()
    }
}


/// Word highlight configuration manager.
#[derive(Debug, Clone)]
pub struct WordhlConfig {
    entries: Vec<WordhlEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single word highlight entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WordhlEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WordhlEntry {
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

impl WordhlConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WordhlEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&WordhlEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WordhlEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WordhlEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&WordhlEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WordhlEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<WordhlEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Word highlight occurrence tracking — extended utilities (qz)
// ---------------------------------------------------------------------------

/// Metric accumulator for wordhl operations.
#[derive(Debug, Clone)]
pub struct QzMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QzMetrics {
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

/// Sliding-window rate counter for wordhl.
#[derive(Debug, Clone)]
pub struct QzRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QzRateWindow {
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

/// A small LRU-style cache for wordhl lookups.
#[derive(Debug, Clone)]
pub struct QzLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QzLruCache {
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
// xb_ utilities – batch 16
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer16 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer16 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_16(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_16<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_16<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_16(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_16(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 238
// ---------------------------------------------------------------------------

/// Generic object pool `Xc238Pool<T>`.
pub struct Xc238Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc238Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc238PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc238Pool<T> {
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
    pub fn stats(&self) -> Xc238PoolStats {
        Xc238PoolStats {
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

impl<T> Default for Xc238Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc238Scheduler`.
pub struct Xc238Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc238Scheduler {
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

impl Default for Xc238Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_238 hash for the given byte slice.
pub fn xc_238_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_238 convention.
pub fn xc_238_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe28 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe28Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe28PipelineError {
    pub stage: Xe28Stage,
    pub message: String,
}

impl std::fmt::Display for Xe28PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe28Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe28Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError>>>,
    stage_names: Vec<Xe28Stage>,
}

impl Xe28Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe28Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe28Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe28Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe28Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe28Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe28CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe28CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe28Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe28CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe28CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe28Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe28CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_28_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe28CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_28_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe28CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_28_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> {
    Ok(data)
}

pub fn xe_28_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_28_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_28_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_28_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe28PipelineError> {
    Err(Xe28PipelineError {
        stage: Xe28Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #114
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf114Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf114TrieNode {
    children: std::collections::HashMap<char, Xf114TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf114Trie {
    root: Xf114TrieNode,
    count: usize,
}

impl Xf114Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf114TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf114TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf114TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf114BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf114BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 237).
pub struct Xh237SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh237SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 279 as u64,
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

/// A compact bit set supporting boolean operations (variant 237).
pub struct Xh237BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh237BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 237).
pub struct Xi237Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi237Deque<T> {
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
pub struct Xi237Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi237Interval {
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

/// A simple interval tree (variant 237).
pub struct Xi237IntervalTree {
    xi_intervals: Vec<Xi237Interval>,
}

impl Xi237IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi237Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi237Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi237Interval) -> Vec<&Xi237Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi237Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi237Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi237Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi237Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi237Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi237Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 237) ---

/// Disjoint set / union-find for crate 237.
pub struct Xj237UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj237UnionFind {
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

const XJ237_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 237.
pub struct Xj237BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj237BTreeNode<K, V>>>,
    len: usize,
}

struct Xj237BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj237BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj237BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ237_BTREE_ORDER - 1
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
        let mid = XJ237_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj237BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj237BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj237BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj237BTreeNode::xj_new_leaf();
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


// --- xk_237 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk237SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk237SegmentTree {
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
pub struct Xk237DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk237DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_237).
#[derive(Debug, Clone)]
pub struct Xl237Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl237Rope {
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

/// Suffix array for efficient string searching (xl_237).
#[derive(Debug, Clone)]
pub struct Xl237SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl237SuffixArray {
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
pub struct Xm237MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm237MatrixSparse {
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
pub struct Xm237Tokenizer {
    text: String,
}

impl Xm237Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 237.
pub struct Xn237Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn237Fenwick {
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

// ----- AVL tree map — crate 237 -----

#[derive(Debug, Clone)]
struct Xn237AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn237AvlNode<K, V>>>,
    right: Option<Box<Xn237AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 237.
#[derive(Debug, Clone)]
pub struct Xn237AVL<K, V> {
    root: Option<Box<Xn237AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn237AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn237AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn237AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn237AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn237AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn237AvlNode<K, V>>) -> Box<Xn237AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn237AvlNode<K, V>>) -> Box<Xn237AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn237AvlNode<K, V>>) -> Box<Xn237AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn237AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn237AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn237AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn237AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn237AvlNode<K, V>>) -> &Xn237AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn237AvlNode<K, V>>) -> (Box<Xn237AvlNode<K, V>>, Option<Box<Xn237AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn237AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn237AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn237AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn237AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn237AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn237AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn237AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo237RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo237Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo237RBNode<K, V> {
    key: K,
    value: V,
    color: Xo237Color,
    left: Option<Box<Xo237RBNode<K, V>>>,
    right: Option<Box<Xo237RBNode<K, V>>>,
}

/// A red-black tree map for crate 237.
#[derive(Debug, Clone)]
pub struct Xo237RedBlack<K, V> {
    root: Option<Box<Xo237RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo237RedBlack<K, V> {
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
            r.color = Xo237Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo237RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo237RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo237RBNode {
                    key, value, color: Xo237Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo237RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo237Color::Red)
    }

    fn xo_balance(mut h: Box<Xo237RBNode<K, V>>) -> Box<Xo237RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo237Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo237RBNode<K, V>>) -> Box<Xo237RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo237Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo237RBNode<K, V>>) -> Box<Xo237RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo237Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo237RBNode<K, V>>) {
        h.color = Xo237Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo237Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo237Color::Black; }
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
            r.color = Xo237Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo237RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo237RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo237RBNode<K, V>) -> (K, V, Option<Box<Xo237RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo237RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo237Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo237RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo237ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 237.
#[derive(Debug, Clone)]
pub struct Xo237ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo237ConsistentHash {
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
            let vkey = format!("{}#xo237#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo237#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 237).
#[derive(Debug)]
pub struct Xp237SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp237Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp237Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp237Node<K, V>>>,
    xp_right: Option<Box<Xp237Node<K, V>>>,
}

impl<K: Ord, V> Xp237Node<K, V> {
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

impl<K: Ord, V> Default for Xp237SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp237SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp237Node<K, V>>>, key: &K) -> Option<Box<Xp237Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp237Node<K, V>>) -> Box<Xp237Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp237Node<K, V>>) -> Box<Xp237Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp237Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp237Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp237Node::xp_new(key, val));
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


// --------------- Xq237Treap ---------------

use std::cmp::Ordering as Xq237Ord;

struct Xq237TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq237TreapNode<K, V>>>,
    right: Option<Box<Xq237TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq237Treap<K, V> {
    root: Option<Box<Xq237TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq237TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_237_size<K, V>(node: &Option<Box<Xq237TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_237_update_size<K, V>(node: &mut Xq237TreapNode<K, V>) {
    node.size = 1 + xq_237_size(&node.left) + xq_237_size(&node.right);
}

fn xq_237_rotate_right<K, V>(mut node: Box<Xq237TreapNode<K, V>>) -> Box<Xq237TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_237_update_size(&mut node);
    left.right = Some(node);
    xq_237_update_size(&mut left);
    left
}

fn xq_237_rotate_left<K, V>(mut node: Box<Xq237TreapNode<K, V>>) -> Box<Xq237TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_237_update_size(&mut node);
    right.left = Some(node);
    xq_237_update_size(&mut right);
    right
}

fn xq_237_insert_node<K: Ord, V>(
    node: Option<Box<Xq237TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq237TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq237TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq237Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq237Ord::Less => {
                let (new_left, old) = xq_237_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_237_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_237_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq237Ord::Greater => {
                let (new_right, old) = xq_237_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_237_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_237_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_237_remove_node<K: Ord, V>(
    node: Option<Box<Xq237TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq237TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq237Ord::Less => {
                let (new_left, old) = xq_237_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_237_update_size(&mut n);
                (Some(n), old)
            }
            Xq237Ord::Greater => {
                let (new_right, old) = xq_237_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_237_update_size(&mut n);
                (Some(n), old)
            }
            Xq237Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_237_rotate_right(n);
                    let (new_right, old) = xq_237_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_237_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_237_rotate_left(n);
                    let (new_left, old) = xq_237_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_237_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_237_find_min<K, V>(node: &Option<Box<Xq237TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_237_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_237_find_max<K, V>(node: &Option<Box<Xq237TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_237_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_237_rank<K: Ord, V>(node: &Option<Box<Xq237TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq237Ord::Less => xq_237_rank(&n.left, key),
            Xq237Ord::Equal => xq_237_size(&n.left),
            Xq237Ord::Greater => 1 + xq_237_size(&n.left) + xq_237_rank(&n.right, key),
        },
    }
}

fn xq_237_kth<K, V>(node: &Option<Box<Xq237TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_237_size(&n.left);
        if k < left_size {
            xq_237_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_237_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_237_in_order<K: Clone, V>(node: &Option<Box<Xq237TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_237_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_237_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq237Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 237 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_237_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq237Ord::Equal => return Some(&n.value),
                Xq237Ord::Less => cur = &n.left,
                Xq237Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_237_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_237_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_237_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_237_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_237_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_237_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_237_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq237VEBTree ---------------

pub struct Xq237VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq237VEBTree>>,
    clusters: Vec<Option<Box<Xq237VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq237VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq237VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq237VEBTree::xq_new(self.sqrt_lo)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_word() {
        let lines = vec!["let x = 1;", "let y = x + 2;"];
        let hl = find_word_highlights(&lines, "x");
        assert_eq!(hl.len(), 2);
        assert_eq!(hl[0].line, 1);
        assert_eq!(hl[1].line, 2);
    }

    #[test]
    fn word_boundary() {
        let lines = vec!["prefix xx suffix"];
        let hl = find_word_highlights(&lines, "xx");
        assert_eq!(hl.len(), 1);
    }

    #[test]
    fn no_partial_match() {
        let lines = vec!["foobar"];
        let hl = find_word_highlights(&lines, "foo");
        assert_eq!(hl.len(), 0);
    }

    #[test]
    fn find_word_at_position_basic() {
        let lines = vec!["let hello = 42;"];
        let result = find_word_at_position(&lines, 1, 6);
        assert!(result.is_some());
        let (word, start, end) = result.unwrap();
        assert_eq!(word, "hello");
        assert_eq!(start, 5);
        assert_eq!(end, 10);
    }

    #[test]
    fn find_word_at_position_start_of_word() {
        let lines = vec!["abc def"];
        let (word, _, _) = find_word_at_position(&lines, 1, 1).unwrap();
        assert_eq!(word, "abc");
    }

    #[test]
    fn find_word_at_position_on_space() {
        let lines = vec!["abc def"];
        assert!(find_word_at_position(&lines, 1, 4).is_none());
    }

    #[test]
    fn find_word_at_position_out_of_bounds() {
        let lines = vec!["abc"];
        assert!(find_word_at_position(&lines, 0, 1).is_none());
        assert!(find_word_at_position(&lines, 2, 1).is_none());
        assert!(find_word_at_position(&lines, 1, 0).is_none());
        assert!(find_word_at_position(&lines, 1, 5).is_none());
    }

    #[test]
    fn highlight_word_occurrences_service() {
        let lines = vec!["let x = 1;", "return x;"];
        let hl = WordHighlightService::highlight_word_occurrences(&lines, 1, 5);
        assert_eq!(hl.len(), 2);
    }

    #[test]
    fn highlight_word_occurrences_no_word() {
        let lines = vec!["= + ;"];
        let hl = WordHighlightService::highlight_word_occurrences(&lines, 1, 1);
        assert!(hl.is_empty());
    }

    #[test]
    fn document_highlight_len() {
        let h = DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Read);
        assert_eq!(h.len(), 5);
        assert!(!h.is_empty());
    }

    #[test]
    fn highlight_kind_alias() {
        let k: HighlightKind = HighlightKind::Write;
        assert_eq!(k, DocumentHighlightKind::Write);
    }

    #[test]
    fn underscore_word_boundary() {
        let lines = vec!["my_var = my_var + 1"];
        let hl = find_word_highlights(&lines, "my_var");
        assert_eq!(hl.len(), 2);
    }

    #[test]
    fn try_find_word_line_out_of_range() {
        let lines = vec!["hello"];
        let err = try_find_word_at_position(&lines, 5, 1).unwrap_err();
        assert_eq!(
            err,
            WordHighlightError::LineOutOfRange { line: 5, max_line: 1 }
        );
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn try_find_word_column_out_of_range() {
        let lines = vec!["hi"];
        let err = try_find_word_at_position(&lines, 1, 10).unwrap_err();
        assert!(matches!(err, WordHighlightError::ColumnOutOfRange { .. }));
    }

    #[test]
    fn try_find_word_not_a_word_char() {
        let lines = vec!["a = b"];
        let err = try_find_word_at_position(&lines, 1, 3).unwrap_err();
        assert!(matches!(err, WordHighlightError::NotAWordChar { .. }));
    }

    #[test]
    fn try_find_word_success() {
        let lines = vec!["fn main() {}"];
        let (word, start, end) = try_find_word_at_position(&lines, 1, 4).unwrap();
        assert_eq!(word, "main");
        assert_eq!(start, 4);
        assert_eq!(end, 8);
    }

    #[test]
    fn highlight_contains_column() {
        let h = DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Read);
        assert!(h.contains_column(5));
        assert!(h.contains_column(9));
        assert!(!h.contains_column(10));
        assert!(!h.contains_column(4));
    }

    #[test]
    fn highlight_overlaps() {
        let a = DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Text);
        let b = DocumentHighlight::new(1, 8, 15, DocumentHighlightKind::Text);
        let c = DocumentHighlight::new(1, 10, 15, DocumentHighlightKind::Text);
        let d = DocumentHighlight::new(2, 5, 10, DocumentHighlightKind::Text);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c)); // adjacent, not overlapping
        assert!(!a.overlaps(&d)); // different lines
    }

    #[test]
    fn highlight_with_kind() {
        let h = DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text);
        let h2 = h.with_kind(DocumentHighlightKind::Write);
        assert_eq!(h2.kind, DocumentHighlightKind::Write);
        assert_eq!(h2.line, 1);
    }

    #[test]
    fn highlight_display() {
        let h = DocumentHighlight::new(3, 5, 10, DocumentHighlightKind::Read);
        assert_eq!(h.to_string(), "3:5..10 (read)");
    }

    #[test]
    fn highlight_kind_display() {
        assert_eq!(DocumentHighlightKind::Text.to_string(), "text");
        assert_eq!(DocumentHighlightKind::Read.to_string(), "read");
        assert_eq!(DocumentHighlightKind::Write.to_string(), "write");
    }

    #[test]
    fn highlight_set_operations() {
        let mut set = HighlightSet::new();
        assert!(set.is_empty());
        set.push(DocumentHighlight::new(1, 1, 4, DocumentHighlightKind::Text));
        set.push(DocumentHighlight::new(1, 8, 11, DocumentHighlightKind::Read));
        set.push(DocumentHighlight::new(3, 2, 6, DocumentHighlightKind::Text));
        assert_eq!(set.len(), 3);
        assert_eq!(set.on_line(1).len(), 2);
        assert_eq!(set.on_line(2).len(), 0);
        assert_eq!(set.by_kind(DocumentHighlightKind::Text).len(), 2);
        assert_eq!(set.distinct_line_count(), 2);
        assert_eq!(set.to_string(), "3 highlights on 2 lines");
    }

    #[test]
    fn highlight_set_sorted() {
        let set = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 10, 15, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
        ]);
        let sorted = set.sorted();
        assert_eq!(sorted[0].line, 1);
        assert_eq!(sorted[0].start_column, 1);
        assert_eq!(sorted[1].line, 1);
        assert_eq!(sorted[1].start_column, 10);
        assert_eq!(sorted[2].line, 3);
    }

    #[test]
    fn error_display_messages() {
        let e1 = WordHighlightError::EmptyWord;
        assert_eq!(e1.to_string(), "empty word");
        let e2 = WordHighlightError::NotAWordChar { line: 2, column: 5 };
        assert!(e2.to_string().contains("2:5"));
    }

    #[test]
    fn compute_stats_empty() {
        let stats = compute_highlight_stats(&[], &[]);
        assert_eq!(stats.total_highlights, 0);
        assert_eq!(stats.unique_words, 0);
        assert_eq!(stats.avg_occurrences, 0.0);
        assert!(stats.most_highlighted_word.is_none());
    }

    #[test]
    fn compute_stats_single_word() {
        let lines = vec!["let x = x + x;"];
        let hl = find_word_highlights(&lines, "x");
        let stats = compute_highlight_stats(&hl, &lines);
        assert_eq!(stats.total_highlights, 3);
        assert_eq!(stats.unique_words, 1);
        assert_eq!(stats.avg_occurrences, 3.0);
        assert_eq!(stats.most_highlighted_word.as_deref(), Some("x"));
    }

    #[test]
    fn compute_stats_multiple_words() {
        let lines = vec!["a b a b a"];
        let mut hl = find_word_highlights(&lines, "a");
        hl.extend(find_word_highlights(&lines, "b"));
        let stats = compute_highlight_stats(&hl, &lines);
        assert_eq!(stats.total_highlights, 5);
        assert_eq!(stats.unique_words, 2);
        assert_eq!(stats.most_highlighted_word.as_deref(), Some("a"));
    }

    #[test]
    fn find_word_boundaries_basic() {
        let boundaries = find_word_boundaries("hello world_2 +foo");
        assert_eq!(boundaries.len(), 3);
        assert_eq!(boundaries[0].word, "hello");
        assert_eq!(boundaries[0].start, 0);
        assert_eq!(boundaries[0].end, 5);
        assert_eq!(boundaries[1].word, "world_2");
        assert_eq!(boundaries[2].word, "foo");
        assert_eq!(boundaries[2].start, 15);
    }

    #[test]
    fn find_word_boundaries_empty() {
        assert!(find_word_boundaries("").is_empty());
        assert!(find_word_boundaries("   +-= ").is_empty());
    }

    #[test]
    fn highlight_merge_no_overlap() {
        let highlights = vec![
            DocumentHighlight::new(1, 1, 4, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 6, 10, DocumentHighlightKind::Read),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Text),
        ];
        let merged = highlight_merge(&highlights);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn highlight_merge_overlapping() {
        let highlights = vec![
            DocumentHighlight::new(1, 1, 8, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 5, 12, DocumentHighlightKind::Read),
            DocumentHighlight::new(1, 11, 15, DocumentHighlightKind::Write),
        ];
        let merged = highlight_merge(&highlights);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_column, 1);
        assert_eq!(merged[0].end_column, 15);
        assert_eq!(merged[0].kind, DocumentHighlightKind::Text);
    }

    #[test]
    fn highlight_merge_adjacent() {
        let highlights = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Text),
        ];
        let merged = highlight_merge(&highlights);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_column, 1);
        assert_eq!(merged[0].end_column, 10);
    }

    #[test]
    fn highlight_merge_different_lines() {
        let highlights = vec![
            DocumentHighlight::new(1, 1, 10, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 10, DocumentHighlightKind::Text),
        ];
        let merged = highlight_merge(&highlights);
        assert_eq!(merged.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Debounce tests
    // -----------------------------------------------------------------------

    #[test]
    fn debounce_new_defaults() {
        let d = HighlightDebounce::default();
        assert_eq!(d.delay_ms, 150);
        assert!(!d.is_pending());
        assert_eq!(d.position(), (0, 0));
    }

    #[test]
    fn debounce_cursor_moved() {
        let mut d = HighlightDebounce::new(100);
        assert!(d.cursor_moved(5, 10, 0));
        assert!(d.is_pending());
        assert_eq!(d.position(), (5, 10));
    }

    #[test]
    fn debounce_same_position_no_change() {
        let mut d = HighlightDebounce::new(100);
        d.cursor_moved(5, 10, 0);
        assert!(!d.cursor_moved(5, 10, 50));
    }

    #[test]
    fn debounce_fires_after_delay() {
        let mut d = HighlightDebounce::new(100);
        d.cursor_moved(5, 10, 0);
        assert!(!d.should_update(50));  // too early
        assert!(d.should_update(100));  // exactly at threshold
        assert!(!d.should_update(200)); // already fired
    }

    #[test]
    fn debounce_cancel() {
        let mut d = HighlightDebounce::new(100);
        d.cursor_moved(5, 10, 0);
        d.cancel();
        assert!(!d.is_pending());
        assert!(!d.should_update(200));
    }

    #[test]
    fn debounce_multiple_moves_resets_timer() {
        let mut d = HighlightDebounce::new(100);
        d.cursor_moved(1, 1, 0);
        d.cursor_moved(2, 2, 80);
        assert!(!d.should_update(100)); // 100 - 80 = 20ms, too early
        assert!(d.should_update(180));  // 180 - 80 = 100ms, fires
    }

    // -----------------------------------------------------------------------
    // Highlight render config tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_config_defaults() {
        let cfg = HighlightRenderConfig::default();
        assert_eq!(cfg.debounce_ms, 150);
        assert_ne!(cfg.read_bg, cfg.write_bg);
    }

    #[test]
    fn render_config_bg_for_kind() {
        let cfg = HighlightRenderConfig::default();
        assert_eq!(cfg.bg_for_kind(DocumentHighlightKind::Read), cfg.read_bg);
        assert_eq!(cfg.bg_for_kind(DocumentHighlightKind::Write), cfg.write_bg);
        assert_eq!(cfg.bg_for_kind(DocumentHighlightKind::Text), cfg.text_bg);
    }

    #[test]
    fn categorize_symbol_keywords() {
        assert_eq!(categorize_symbol("fn"), SymbolCategory::Keyword);
        assert_eq!(categorize_symbol("let"), SymbolCategory::Keyword);
        assert_eq!(categorize_symbol("return"), SymbolCategory::Keyword);
    }

    #[test]
    fn categorize_symbol_types_and_variables() {
        assert_eq!(categorize_symbol("MyStruct"), SymbolCategory::Type);
        assert_eq!(categorize_symbol("my_var"), SymbolCategory::Variable);
        assert_eq!(categorize_symbol("123"), SymbolCategory::Literal);
    }

    #[test]
    fn symbol_category_display() {
        assert_eq!(SymbolCategory::Variable.to_string(), "variable");
        assert_eq!(SymbolCategory::Function.to_string(), "function");
        assert_eq!(SymbolCategory::Type.to_string(), "type");
        assert_eq!(SymbolCategory::Keyword.to_string(), "keyword");
        assert_eq!(SymbolCategory::Literal.to_string(), "literal");
        assert_eq!(SymbolCategory::Unknown.to_string(), "unknown");
    }

    #[test]
    fn prioritized_highlight_sorting() {
        let mut phs = vec![
            PrioritizedHighlight {
                highlight: DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
                priority: HighlightPriority::LOW,
            },
            PrioritizedHighlight {
                highlight: DocumentHighlight::new(1, 10, 15, DocumentHighlightKind::Read),
                priority: HighlightPriority::HIGH,
            },
            PrioritizedHighlight {
                highlight: DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Write),
                priority: HighlightPriority::NORMAL,
            },
        ];
        sort_by_priority(&mut phs);
        assert_eq!(phs[0].priority, HighlightPriority::HIGH);
        assert_eq!(phs[1].priority, HighlightPriority::NORMAL);
        assert_eq!(phs[2].priority, HighlightPriority::LOW);
    }

    #[test]
    fn multi_word_tracker_operations() {
        let mut tracker = MultiWordTracker::new();
        assert_eq!(tracker.word_count(), 0);
        assert_eq!(tracker.total_highlights(), 0);

        let mut set1 = HighlightSet::new();
        set1.push(DocumentHighlight::new(1, 1, 4, DocumentHighlightKind::Text));
        set1.push(DocumentHighlight::new(2, 1, 4, DocumentHighlightKind::Read));
        tracker.set_word("foo".to_string(), set1);
        assert_eq!(tracker.word_count(), 1);
        assert_eq!(tracker.total_highlights(), 2);
        assert!(tracker.get_word("foo").is_some());
        assert!(tracker.get_word("bar").is_none());

        assert!(tracker.remove_word("foo"));
        assert!(!tracker.remove_word("foo"));
        assert_eq!(tracker.word_count(), 0);
    }

    #[test]
    fn multi_word_tracker_replace_and_clear() {
        let mut tracker = MultiWordTracker::new();
        let set1 = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
        ]);
        tracker.set_word("x".to_string(), set1);
        assert_eq!(tracker.total_highlights(), 1);

        let set2 = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Text),
        ]);
        tracker.set_word("x".to_string(), set2);
        assert_eq!(tracker.word_count(), 1);
        assert_eq!(tracker.total_highlights(), 2);

        tracker.clear();
        assert_eq!(tracker.word_count(), 0);
    }

    #[test]
    fn highlight_priority_ordering() {
        assert!(HighlightPriority::HIGH > HighlightPriority::NORMAL);
        assert!(HighlightPriority::NORMAL > HighlightPriority::LOW);
        assert_eq!(HighlightPriority(50), HighlightPriority::NORMAL);
    }

    #[test]
    fn highlight_span() {
        let h = DocumentHighlight::new(1, 3, 10, DocumentHighlightKind::Text);
        assert_eq!(h.span(), 7);
        let empty = DocumentHighlight::new(1, 5, 5, DocumentHighlightKind::Read);
        assert_eq!(empty.span(), 0);
    }

    #[test]
    fn highlight_kind_label() {
        assert_eq!(DocumentHighlightKind::Read.label(), "read");
        assert_eq!(DocumentHighlightKind::Write.label(), "write");
        assert_eq!(DocumentHighlightKind::Text.label(), "text");
    }

    #[test]
    fn highlight_set_merge() {
        let mut a = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
        ]);
        let b = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Write),
        ]);
        a.merge(b);
        assert_eq!(a.len(), 3);
        assert_eq!(a.distinct_line_count(), 3);
    }

    #[test]
    fn highlight_set_remove_on_line() {
        let mut set = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 8, 12, DocumentHighlightKind::Read),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Text),
        ]);
        let removed = set.remove_on_line(1);
        assert_eq!(removed, 2);
        assert_eq!(set.len(), 1);
        assert_eq!(set.remove_on_line(99), 0);
    }

    #[test]
    fn highlight_set_clear() {
        let mut set = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Read),
        ]);
        assert!(!set.is_empty());
        set.clear();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn highlight_set_lines() {
        let set = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(5, 1, 3, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 3, DocumentHighlightKind::Read),
            DocumentHighlight::new(5, 6, 9, DocumentHighlightKind::Write),
            DocumentHighlight::new(1, 1, 3, DocumentHighlightKind::Text),
        ]);
        assert_eq!(set.lines(), vec![1, 2, 5]);
    }

    #[test]
    fn highlight_set_contains_position() {
        let set = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 2, 6, DocumentHighlightKind::Text),
        ]);
        assert!(set.contains_position(1, 5));
        assert!(set.contains_position(1, 9));
        assert!(!set.contains_position(1, 10)); // exclusive end
        assert!(!set.contains_position(1, 4));
        assert!(set.contains_position(3, 3));
        assert!(!set.contains_position(2, 5)); // wrong line
    }

    #[test]
    fn highlight_set_display_format() {
        let empty = HighlightSet::new();
        assert_eq!(empty.to_string(), "0 highlights on 0 lines");

        let set = HighlightSet::from_highlights(vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 8, 12, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Write),
        ]);
        assert_eq!(set.to_string(), "3 highlights on 2 lines");
    }

    // -- HighlightMerger tests -----------------------------------------------

    #[test]
    fn merger_combines_overlapping() {
        let mut merger = HighlightMerger::new();
        merger.push(DocumentHighlight::new(1, 0, 5, DocumentHighlightKind::Text));
        merger.push(DocumentHighlight::new(1, 3, 8, DocumentHighlightKind::Read));
        let merged = merger.merge();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_column, 0);
        assert_eq!(merged[0].end_column, 8);
        assert_eq!(merged[0].kind, DocumentHighlightKind::Read);
    }

    #[test]
    fn merger_keeps_non_overlapping_separate() {
        let merger = HighlightMerger::from_highlights(vec![
            DocumentHighlight::new(1, 0, 3, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 5, 8, DocumentHighlightKind::Text),
        ]);
        let merged = merger.merge();
        assert_eq!(merged.len(), 2);
    }

    // -- HighlightDiff tests -------------------------------------------------

    #[test]
    fn diff_detects_added_and_removed() {
        let before = vec![
            DocumentHighlight::new(1, 0, 3, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 0, 3, DocumentHighlightKind::Read),
        ];
        let after = vec![
            DocumentHighlight::new(2, 0, 3, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 0, 3, DocumentHighlightKind::Write),
        ];
        let diff = diff_highlights(&before, &after);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.added[0].line, 3);
        assert_eq!(diff.removed[0].line, 1);
    }

    #[test]
    fn diff_empty_when_identical() {
        let set = vec![DocumentHighlight::new(1, 0, 5, DocumentHighlightKind::Text)];
        let diff = diff_highlights(&set, &set);
        assert!(diff.is_empty());
        assert_eq!(diff.change_count(), 0);
    }

    // -- WordOccurrenceTracker tests -----------------------------------------

    #[test]
    fn tracker_counts_word_occurrences() {
        let lines = vec!["let x = x + 1;", "let y = x;"];
        let mut tracker = WordOccurrenceTracker::new();
        tracker.track_word(&lines, "x");
        assert_eq!(tracker.count("x"), 3);
        assert_eq!(tracker.total_occurrences(), 3);

        let positions = tracker.get("x");
        assert_eq!(positions[0], WordOccurrence { line: 0, column: 4, length: 1 });
        assert_eq!(positions[1], WordOccurrence { line: 0, column: 8, length: 1 });
        assert_eq!(positions[2], WordOccurrence { line: 1, column: 8, length: 1 });
    }

    #[test]
    fn tracker_lines_with_word() {
        let lines = vec!["hello world", "foo bar", "hello again"];
        let mut tracker = WordOccurrenceTracker::new();
        tracker.track_word(&lines, "hello");
        let word_lines = tracker.lines_with_word("hello");
        assert_eq!(word_lines, vec![0, 2]);
    }

    #[test]
    fn filter_highlights_on_lines_basic() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Write),
        ];
        let filtered = filter_highlights_on_lines(&hl, &[1, 3]);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].line, 1);
        assert_eq!(filtered[1].line, 3);
    }

    #[test]
    fn filter_highlights_on_lines_empty() {
        let hl: Vec<DocumentHighlight> = vec![];
        assert!(filter_highlights_on_lines(&hl, &[1]).is_empty());
    }

    #[test]
    fn count_by_kind_mixed() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 6, 10, DocumentHighlightKind::Read),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Write),
            DocumentHighlight::new(2, 6, 10, DocumentHighlightKind::Read),
        ];
        let (t, r, w) = count_by_kind(&hl);
        assert_eq!(t, 1);
        assert_eq!(r, 2);
        assert_eq!(w, 1);
    }

    #[test]
    fn total_highlight_span_computed() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 10, 15, DocumentHighlightKind::Text),
        ];
        assert_eq!(total_highlight_span(&hl), 9); // 4 + 5
    }

    #[test]
    fn highlights_at_position_finds() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 3, 8, DocumentHighlightKind::Read),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Write),
        ];
        let found = highlights_at_position(&hl, 1, 4);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn has_overlapping_highlights_true() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 3, 8, DocumentHighlightKind::Read),
        ];
        assert!(has_overlapping_highlights(&hl));
    }

    #[test]
    fn has_overlapping_highlights_false() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 6, 10, DocumentHighlightKind::Read),
        ];
        assert!(!has_overlapping_highlights(&hl));
    }

    #[test]
    fn lines_with_writes_returns_sorted() {
        let hl = vec![
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Write),
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 6, 10, DocumentHighlightKind::Write),
        ];
        assert_eq!(lines_with_writes(&hl), vec![1, 3]);
    }

    #[test]
    fn lines_with_writes_empty() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
        ];
        assert!(lines_with_writes(&hl).is_empty());
    }

    #[test]
    fn filter_by_kind_text() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Write),
        ];
        let text_only = filter_by_kind(&hl, DocumentHighlightKind::Text);
        assert_eq!(text_only.len(), 1);
        assert_eq!(text_only[0].line, 1);
    }

    #[test]
    fn count_by_kind_counts() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Read),
            DocumentHighlight::new(4, 1, 5, DocumentHighlightKind::Write),
        ];
        assert_eq!(count_by_kind(&hl), (2, 1, 1));
    }

    #[test]
    fn highlights_at_finds_match() {
        let hl = vec![
            DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 15, 20, DocumentHighlightKind::Read),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Text),
        ];
        let at_1_7 = highlights_at(&hl, 1, 7);
        assert_eq!(at_1_7.len(), 1);
        assert_eq!(at_1_7[0].start_column, 5);
    }

    #[test]
    fn sort_highlights_orders_by_position() {
        let mut hl = vec![
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 10, 15, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
        ];
        sort_highlights(&mut hl);
        assert_eq!(hl[0].line, 1);
        assert_eq!(hl[0].start_column, 1);
        assert_eq!(hl[1].start_column, 10);
        assert_eq!(hl[2].line, 3);
    }

    #[test]
    fn remove_subsumed_highlights_removes_inner() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 20, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Read),
        ];
        let result = remove_subsumed_highlights(&hl);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_column, 1);
        assert_eq!(result[0].end_column, 20);
    }

    #[test]
    fn highlighted_line_count_unique() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(1, 6, 10, DocumentHighlightKind::Text),
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Text),
        ];
        assert_eq!(highlighted_line_count(&hl), 2);
    }

    #[test]
    fn widest_highlight_returns_longest() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 20, DocumentHighlightKind::Text),
            DocumentHighlight::new(3, 1, 10, DocumentHighlightKind::Text),
        ];
        let w = widest_highlight(&hl).unwrap();
        assert_eq!(w.line, 2);
        assert_eq!(w.span(), 19);
    }

    #[test]
    fn highlight_summary_format() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Write),
        ];
        assert_eq!(highlight_summary(&hl), "1 text, 0 read, 1 write");
    }

    #[test]
    fn highlight_full_line_creates() {
        let h = highlight_full_line(5, "hello world", DocumentHighlightKind::Read);
        assert_eq!(h.line, 5);
        assert_eq!(h.start_column, 1);
        assert_eq!(h.end_column, 12);
        assert_eq!(h.kind, DocumentHighlightKind::Read);
    }

    #[test]
    fn test_navigation_next_wraps() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 4, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 4, DocumentHighlightKind::Text),
        ];
        let mut nav = WordHighlightNavigation::new(hl);
        assert_eq!(nav.current().unwrap().line, 1);
        nav.next();
        assert_eq!(nav.current().unwrap().line, 2);
        nav.next();
        assert_eq!(nav.current().unwrap().line, 1); // wrapped
    }

    #[test]
    fn test_navigation_previous_wraps() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 4, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 4, DocumentHighlightKind::Text),
            DocumentHighlight::new(3, 1, 4, DocumentHighlightKind::Text),
        ];
        let mut nav = WordHighlightNavigation::new(hl);
        nav.previous(); // wraps from 0 to last
        assert_eq!(nav.current().unwrap().line, 3);
    }

    #[test]
    fn test_navigation_position_label() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 4, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 5, 8, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 1, 3, DocumentHighlightKind::Write),
        ];
        let mut nav = WordHighlightNavigation::new(hl);
        assert_eq!(nav.position_label(), "1 of 3");
        nav.next();
        assert_eq!(nav.position_label(), "2 of 3");
        nav.next();
        assert_eq!(nav.position_label(), "3 of 3");
    }

    #[test]
    fn test_navigation_empty() {
        let mut nav = WordHighlightNavigation::new(vec![]);
        assert!(nav.current().is_none());
        assert!(nav.next().is_none());
        assert!(nav.previous().is_none());
        assert_eq!(nav.count(), 0);
        assert_eq!(nav.position_label(), "0 of 0");
    }

    #[test]
    fn test_navigation_reset() {
        let hl = vec![
            DocumentHighlight::new(1, 1, 4, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 4, DocumentHighlightKind::Text),
        ];
        let mut nav = WordHighlightNavigation::new(hl);
        nav.next();
        nav.next();
        assert_eq!(nav.current().unwrap().line, 1);
        nav.next();
        assert_eq!(nav.current().unwrap().line, 2);
        nav.reset();
        assert_eq!(nav.current().unwrap().line, 1);
    }

    #[test]
    fn test_scope_contains_line() {
        let scope = WordHighlightScope::new(5, 15);
        assert!(!scope.contains_line(4));
        assert!(scope.contains_line(5));
        assert!(scope.contains_line(10));
        assert!(scope.contains_line(15));
        assert!(!scope.contains_line(16));
    }

    #[test]
    fn test_scope_filter() {
        let scope = WordHighlightScope::new(2, 4);
        let hl = vec![
            DocumentHighlight::new(1, 1, 5, DocumentHighlightKind::Text),
            DocumentHighlight::new(2, 1, 5, DocumentHighlightKind::Read),
            DocumentHighlight::new(3, 1, 5, DocumentHighlightKind::Write),
            DocumentHighlight::new(5, 1, 5, DocumentHighlightKind::Text),
        ];
        let filtered = scope.filter(&hl);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].line, 2);
        assert_eq!(filtered[1].line, 3);
    }

    #[test]
    fn test_scope_expand_shrink() {
        let mut scope = WordHighlightScope::new(10, 20);
        assert_eq!(scope.line_count(), 11);
        scope.expand(5);
        assert_eq!(scope.start_line, 5);
        assert_eq!(scope.end_line, 25);
        scope.shrink(3);
        assert_eq!(scope.start_line, 8);
        assert_eq!(scope.end_line, 22);
        // Shrink a lot — start and end should not cross
        let mut small = WordHighlightScope::new(10, 12);
        small.shrink(5);
        assert!(small.start_line <= small.end_line);
    }

    #[test]
    fn test_semantic_provider_case_insensitive() {
        let provider = SemanticHighlightProvider::new().with_case_sensitive(false);
        let lines = vec!["let Foo = 1;", "let foo = 2;", "let FOO = 3;"];
        let hl = provider.find_highlights(&lines, "foo");
        assert_eq!(hl.len(), 3);
    }

    #[test]
    fn test_semantic_provider_whole_word() {
        let provider = SemanticHighlightProvider::new().with_whole_word(false);
        let lines = vec!["foobar foo barfoo"];
        let hl = provider.find_highlights(&lines, "foo");
        assert_eq!(hl.len(), 3); // foobar, foo, barfoo

        let strict = SemanticHighlightProvider::new().with_whole_word(true);
        let hl2 = strict.find_highlights(&lines, "foo");
        assert_eq!(hl2.len(), 1); // only standalone "foo"
    }

    #[test]
    fn test_semantic_provider_categories() {
        let provider = SemanticHighlightProvider::new()
            .with_categories(vec![SymbolCategory::Keyword, SymbolCategory::Type]);
        assert!(provider.matches_category("fn"));
        assert!(provider.matches_category("MyType"));
        assert!(!provider.matches_category("my_var"));

        let empty = SemanticHighlightProvider::new();
        assert!(empty.matches_category("anything"));
    }

    #[test]
    fn test_throttler_timing() {
        let mut throttler = HighlightThrottler::new(100);
        // First request at t=100 should process (100 - 0 >= 100)
        assert!(throttler.should_process(100));
        assert!(throttler.request("hello".into(), 100));
        // At t=150, not enough time has passed
        assert!(!throttler.should_process(150));
        assert!(!throttler.request("world".into(), 150));
        assert_eq!(throttler.take_pending(), Some("world".into()));
        assert!(throttler.take_pending().is_none());
        // At t=200, enough time has passed
        assert!(throttler.should_process(200));
        assert!(throttler.request("again".into(), 200));
        throttler.clear();
        assert!(throttler.pending.is_none());
    }

    #[test]
    fn wordHighlightSemanticFilter_new() {
        let s = WordHighlightSemanticFilter::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn wordHighlightSemanticFilter_add_contains() {
        let mut s = WordHighlightSemanticFilter::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn wordHighlightSemanticFilter_add_duplicate() {
        let mut s = WordHighlightSemanticFilter::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn wordHighlightSemanticFilter_remove() {
        let mut s = WordHighlightSemanticFilter::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn wordHighlightSemanticFilter_capacity() {
        let s = WordHighlightSemanticFilter::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn wordHighlightSemanticFilter_search() {
        let mut s = WordHighlightSemanticFilter::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn wordHighlightSemanticFilter_stats() {
        let mut s = WordHighlightSemanticFilter::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn wordHighlightAnimation_new() {
        let m = WordHighlightAnimation::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn wordHighlightAnimation_add_find() {
        let mut m = WordHighlightAnimation::new();
        m.add(WordHighlightAnimationItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn wordHighlightAnimation_priority_filter() {
        let mut m = WordHighlightAnimation::new();
        m.add(WordHighlightAnimationItem::new("a", "A").with_priority(WordHighlightAnimationPriority::High));
        m.add(WordHighlightAnimationItem::new("b", "B").with_priority(WordHighlightAnimationPriority::Low));
        m.add(WordHighlightAnimationItem::new("c", "C").with_priority(WordHighlightAnimationPriority::High));
        assert_eq!(m.by_priority(WordHighlightAnimationPriority::High).len(), 2);
    }

    #[test]
    fn wordHighlightAnimation_remove() {
        let mut m = WordHighlightAnimation::new();
        m.add(WordHighlightAnimationItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn wordHighlightAnimation_search() {
        let mut m = WordHighlightAnimation::new();
        m.add(WordHighlightAnimationItem::new("id1", "Hello World"));
        m.add(WordHighlightAnimationItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn wordHighlightAnimation_total_weight() {
        let mut m = WordHighlightAnimation::new();
        m.add(WordHighlightAnimationItem::new("a", "A").with_priority(WordHighlightAnimationPriority::Critical));
        m.add(WordHighlightAnimationItem::new("b", "B").with_priority(WordHighlightAnimationPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn wordHighlightAnimation_capacity_limit() {
        let mut m = WordHighlightAnimation::new().with_max_items(2);
        m.add(WordHighlightAnimationItem::new("1", "one"));
        m.add(WordHighlightAnimationItem::new("2", "two"));
        assert!(!m.add(WordHighlightAnimationItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn wordHighlightAnimation_sorted_by_priority() {
        let mut m = WordHighlightAnimation::new();
        m.add(WordHighlightAnimationItem::new("lo", "Low").with_priority(WordHighlightAnimationPriority::Low));
        m.add(WordHighlightAnimationItem::new("hi", "High").with_priority(WordHighlightAnimationPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn wordHighlightAnimation_item_metadata() {
        let mut item = WordHighlightAnimationItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn wordHighlightSemanticFilter_enabled_toggle() {
        let mut s = WordHighlightSemanticFilter::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn wordHighlightAnimation_priority_display() {
        assert_eq!(format!("{}", WordHighlightAnimationPriority::High), "high");
        assert_eq!(format!("{}", WordHighlightAnimationPriority::Low), "low");
    }


    #[test]
    fn wordhl_entry_creation() {
        let e = WordhlEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wordhl_entry_with_priority() {
        let e = WordhlEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wordhl_entry_metadata() {
        let e = WordhlEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wordhl_entry_remove_meta() {
        let mut e = WordhlEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wordhl_entry_activate_deactivate() {
        let mut e = WordhlEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wordhl_config_add_sorted() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("lo", "Lo").with_priority(1));
        c.add(WordhlEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wordhl_config_capacity() {
        let mut c = WordhlConfig::new(1);
        assert!(c.add(WordhlEntry::new("a", "A")));
        assert!(!c.add(WordhlEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wordhl_config_remove() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wordhl_config_get() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wordhl_config_active_entries() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "A"));
        c.add(WordhlEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wordhl_config_enable_disable() {
        let mut c = WordhlConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wordhl_config_clear() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wordhl_config_find_by_label() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wordhl_config_top_n() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "A").with_priority(1));
        c.add(WordhlEntry::new("b", "B").with_priority(2));
        c.add(WordhlEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wordhl_config_deactivate_activate_all() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "A"));
        c.add(WordhlEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wordhl_config_highest_priority() {
        let mut c = WordhlConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WordhlEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wordhl_config_contains() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wordhl_config_labels() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "Alpha"));
        c.add(WordhlEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wordhl_config_drain_inactive() {
        let mut c = WordhlConfig::new(10);
        c.add(WordhlEntry::new("a", "A"));
        c.add(WordhlEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qz_metrics_empty() {
        let m = QzMetrics::new("wordhl");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qz_metrics_record_and_mean() {
        let mut m = QzMetrics::new("wordhl");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qz_metrics_min_max() {
        let mut m = QzMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qz_metrics_variance_and_std() {
        let mut m = QzMetrics::new("v");
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
    fn qz_metrics_percentile() {
        let mut m = QzMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qz_metrics_merge() {
        let mut a = QzMetrics::new("a");
        a.record(1.0);
        let mut b = QzMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qz_metrics_reset() {
        let mut m = QzMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qz_rate_window_empty() {
        let rw = QzRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qz_rate_window_tick_and_rate() {
        let mut rw = QzRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qz_lru_cache_basic() {
        let mut c = QzLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qz_lru_cache_contains_and_keys() {
        let mut c = QzLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qz_lru_cache_remove() {
        let mut c = QzLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qz_metrics_sum() {
        let mut m = QzMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qz_metrics_label() {
        let m = QzMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qz_lru_cache_clear() {
        let mut c = QzLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_16_push_and_len() {
        let mut rb = super::XbRingBuffer16::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_16_overwrite() {
        let mut rb = super::XbRingBuffer16::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_16_get_out_of_bounds() {
        let rb = super::XbRingBuffer16::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_16_drain_all() {
        let mut rb = super::XbRingBuffer16::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_16_peek_front_back() {
        let mut rb = super::XbRingBuffer16::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_16_clear() {
        let mut rb = super::XbRingBuffer16::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_16_capacity() {
        let rb = super::XbRingBuffer16::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_16_basic() {
        let h = super::xb_fnv1a_16(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_16(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_16_different_inputs() {
        let h1 = super::xb_fnv1a_16(b"abc");
        let h2 = super::xb_fnv1a_16(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_16_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_16(&data);
        let dec = super::xb_rle_decode_16(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_16_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_16(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_16(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_16_values() {
        assert!((super::xb_clamp_16(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_16(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_16(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_16_values() {
        assert!((super::xb_lerp_16(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_16(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_16(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_16_wrap_around_twice() {
        let mut rb = super::XbRingBuffer16::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 238 ----

    #[test]
    fn xc_238_pool_new_empty() {
        let pool: super::Xc238Pool<i32> = super::Xc238Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_238_pool_release_acquire() {
        let mut pool = super::Xc238Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_238_pool_acquire_empty() {
        let mut pool: super::Xc238Pool<i32> = super::Xc238Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_238_pool_full() {
        let mut pool = super::Xc238Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_238_pool_drain() {
        let mut pool = super::Xc238Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_238_pool_stats() {
        let mut pool = super::Xc238Pool::new(8);
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
    fn xc_238_pool_clear() {
        let mut pool = super::Xc238Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_238_pool_shrink() {
        let mut pool = super::Xc238Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_238_pool_default() {
        let pool: super::Xc238Pool<String> = super::Xc238Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_238_pool_extend() {
        let mut pool = super::Xc238Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_238_pool_retain() {
        let mut pool = super::Xc238Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_238_scheduler_round_robin() {
        let mut sched = super::Xc238Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_238_scheduler_empty() {
        let mut sched = super::Xc238Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_238_scheduler_reset() {
        let mut sched = super::Xc238Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_238_scheduler_add_remove() {
        let mut sched = super::Xc238Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_238_scheduler_targets() {
        let sched = super::Xc238Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_238_hash_empty() {
        assert_eq!(super::xc_238_hash(b""), 5381);
    }

    #[test]
    fn xc_238_hash_data() {
        let h = super::xc_238_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_238_hash(b"hello"), h);
    }

    #[test]
    fn xc_238_reverse_str() {
        assert_eq!(super::xc_238_reverse("abc"), "cba");
        assert_eq!(super::xc_238_reverse(""), "");
    }


    #[test]
    fn xe_28_pipeline_empty() {
        let p = super::Xe28Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_28_pipeline_parse_stage() {
        let p = super::Xe28Pipeline::new()
            .add_parse(super::xe_28_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_28_pipeline_transform_double() {
        let p = super::Xe28Pipeline::new()
            .add_transform(super::xe_28_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_28_pipeline_validate_reverse() {
        let p = super::Xe28Pipeline::new()
            .add_validate(super::xe_28_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_28_pipeline_emit_filter() {
        let p = super::Xe28Pipeline::new()
            .add_emit(super::xe_28_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_28_pipeline_multi_stage() {
        let p = super::Xe28Pipeline::new()
            .add_parse(super::xe_28_pipeline_identity)
            .add_transform(super::xe_28_pipeline_double)
            .add_validate(super::xe_28_pipeline_reverse)
            .add_emit(super::xe_28_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_28_pipeline_error_propagation() {
        let p = super::Xe28Pipeline::new()
            .add_parse(super::xe_28_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe28Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_28_pipeline_compose() {
        let p1 = super::Xe28Pipeline::new()
            .add_parse(super::xe_28_pipeline_identity);
        let p2 = super::Xe28Pipeline::new()
            .add_transform(super::xe_28_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_28_pipeline_error_display() {
        let e = super::Xe28PipelineError {
            stage: super::Xe28Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_28_cache_put_get() {
        let mut c = super::Xe28Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_28_cache_miss() {
        let mut c: super::Xe28Cache<&str, i32> = super::Xe28Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_28_cache_ttl_expiry() {
        let mut c = super::Xe28Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_28_cache_evict() {
        let mut c = super::Xe28Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_28_cache_capacity() {
        let mut c = super::Xe28Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_28_cache_stats() {
        let mut c = super::Xe28Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_28_cache_clear() {
        let mut c = super::Xe28Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #114 --

    #[test]
    fn xf114_trie_insert_search() {
        let mut t = Xf114Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf114_trie_starts_with() {
        let mut t = Xf114Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf114_trie_remove() {
        let mut t = Xf114Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf114_trie_word_count() {
        let mut t = Xf114Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf114_trie_longest_prefix() {
        let mut t = Xf114Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf114_trie_all_words() {
        let mut t = Xf114Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf114_trie_autocomplete() {
        let mut t = Xf114Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf114_trie_empty_search() {
        let t = Xf114Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf114_bloom_add_contains() {
        let mut bf = Xf114BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf114_bloom_probably_absent() {
        let bf = Xf114BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf114_bloom_false_positive_rate() {
        let mut bf = Xf114BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf114_bloom_clear() {
        let mut bf = Xf114BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf114_bloom_union() {
        let mut a = Xf114BloomFilter::xf_new(512, 2);
        let mut b = Xf114BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf114_bloom_intersection_estimate() {
        let mut a = Xf114BloomFilter::xf_new(512, 2);
        let mut b = Xf114BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf114_bloom_union_size_mismatch() {
        let a = Xf114BloomFilter::xf_new(256, 2);
        let b = Xf114BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh237_skip_insert_contains() {
        let mut sl = super::Xh237SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh237_skip_remove() {
        let mut sl = super::Xh237SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh237_skip_len() {
        let mut sl = super::Xh237SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh237_skip_range_query() {
        let mut sl = super::Xh237SkipList::xh_new(4);
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
    fn xh237_skip_floor_ceiling() {
        let mut sl = super::Xh237SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh237_skip_rank() {
        let mut sl = super::Xh237SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh237_skip_empty() {
        let sl = super::Xh237SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh237_skip_duplicates() {
        let mut sl = super::Xh237SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh237_bitset_set_test() {
        let mut bs = super::Xh237BitSet::xh_new(256);
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
    fn xh237_bitset_clear_count() {
        let mut bs = super::Xh237BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh237_bitset_and_or_xor() {
        let mut a = super::Xh237BitSet::xh_new(128);
        let mut b = super::Xh237BitSet::xh_new(128);
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
    fn xh237_bitset_iter_ones() {
        let mut bs = super::Xh237BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh237_bitset_first_last() {
        let mut bs = super::Xh237BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh237_bitset_empty() {
        let bs = super::Xh237BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi237_deque_push_pop_back() {
        let mut dq = super::Xi237Deque::xi_new(4);
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
    fn xi237_deque_push_pop_front() {
        let mut dq = super::Xi237Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi237_deque_mixed_ops() {
        let mut dq = super::Xi237Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi237_deque_get_and_split() {
        let mut dq = super::Xi237Deque::xi_new(8);
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
    fn xi237_deque_rotate_left() {
        let mut dq = super::Xi237Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi237_deque_rotate_right() {
        let mut dq = super::Xi237Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi237_deque_grow() {
        let mut dq = super::Xi237Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi237_deque_empty() {
        let dq = super::Xi237Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi237_interval_tree_insert_query() {
        let mut tree = super::Xi237IntervalTree::xi_new();
        tree.xi_insert(super::Xi237Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi237Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi237Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi237_interval_tree_overlap() {
        let mut tree = super::Xi237IntervalTree::xi_new();
        tree.xi_insert(super::Xi237Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi237Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi237Interval::xi_new(12, 20));
        let q = super::Xi237Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi237_interval_tree_remove() {
        let mut tree = super::Xi237IntervalTree::xi_new();
        tree.xi_insert(super::Xi237Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi237Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi237_interval_tree_gaps() {
        let mut tree = super::Xi237IntervalTree::xi_new();
        tree.xi_insert(super::Xi237Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi237Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi237Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi237Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi237Interval::xi_new(8, 10));
    }

    #[test]
    fn xi237_interval_tree_merge() {
        let mut tree = super::Xi237IntervalTree::xi_new();
        tree.xi_insert(super::Xi237Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi237Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi237Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi237Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi237Interval::xi_new(10, 15));
    }

    #[test]
    fn xi237_interval_tree_all() {
        let mut tree = super::Xi237IntervalTree::xi_new();
        tree.xi_insert(super::Xi237Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi237Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi237_interval_tree_empty() {
        let tree = super::Xi237IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi237_interval_tree_contains_point() {
        let iv = super::Xi237Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 237) ---

    #[test]
    fn xj_237_uf_make_and_find() {
        let mut uf = super::Xj237UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_237_uf_union_connected() {
        let mut uf = super::Xj237UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_237_uf_component_count() {
        let mut uf = super::Xj237UnionFind::xj_new();
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
    fn xj_237_uf_component_size() {
        let mut uf = super::Xj237UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_237_uf_largest_component() {
        let mut uf = super::Xj237UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_237_uf_many_elements() {
        let mut uf = super::Xj237UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_237_uf_separate_components() {
        let mut uf = super::Xj237UnionFind::xj_new();
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
    fn xj_237_uf_path_compression() {
        let mut uf = super::Xj237UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_237_bt_insert_get() {
        let mut bt = super::Xj237BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_237_bt_contains_len() {
        let mut bt = super::Xj237BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_237_bt_replace() {
        let mut bt = super::Xj237BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_237_bt_remove() {
        let mut bt = super::Xj237BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_237_bt_keys_values() {
        let mut bt = super::Xj237BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_237_bt_range() {
        let mut bt = super::Xj237BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_237_bt_min_max() {
        let mut bt = super::Xj237BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_237_bt_many_inserts() {
        let mut bt = super::Xj237BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_237 segment tree tests ---

    #[test]
    fn xk_237_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk237SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_237_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk237SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_237_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk237SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_237_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk237SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_237_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk237SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_237_st_single_element() {
        let data = vec![42];
        let st = super::Xk237SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_237_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk237SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_237_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk237SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_237 disjoint intervals tests ---

    #[test]
    fn xk_237_di_add_and_count() {
        let mut di = super::Xk237DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_237_di_merge_overlap() {
        let mut di = super::Xk237DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_237_di_contains() {
        let mut di = super::Xk237DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_237_di_remove() {
        let mut di = super::Xk237DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_237_di_covered_length() {
        let mut di = super::Xk237DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_237_di_gaps() {
        let mut di = super::Xk237DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_237_di_merge_adjacent() {
        let mut di = super::Xk237DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_237_di_empty() {
        let di = super::Xk237DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_237_rope_new_empty() {
        let rope = super::Xl237Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_237_rope_from_str() {
        let rope = super::Xl237Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_237_rope_insert_at() {
        let mut rope = super::Xl237Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_237_rope_delete_range() {
        let mut rope = super::Xl237Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_237_rope_char_at() {
        let rope = super::Xl237Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_237_rope_split_concat() {
        let rope = super::Xl237Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_237_rope_line_count() {
        let rope = super::Xl237Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_237_rope_line_at() {
        let rope = super::Xl237Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_237_sa_build_and_search() {
        let sa = super::Xl237SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_237_sa_count() {
        let sa = super::Xl237SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_237_sa_longest_repeated() {
        let sa = super::Xl237SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_237_sa_all_positions() {
        let sa = super::Xl237SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_237_sa_len() {
        let sa = super::Xl237SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_237_sa_empty() {
        let sa = super::Xl237SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_237_rope_slice() {
        let rope = super::Xl237Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_237_sa_search_start() {
        let sa = super::Xl237SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_237_sparse_set_get() {
        let mut m = super::Xm237MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_237_sparse_row_col() {
        let mut m = super::Xm237MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_237_sparse_transpose() {
        let mut m = super::Xm237MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_237_sparse_multiply_vec() {
        let mut m = super::Xm237MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_237_sparse_nnz_density() {
        let mut m = super::Xm237MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_237_sparse_clear() {
        let mut m = super::Xm237MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_237_sparse_overwrite_zero() {
        let mut m = super::Xm237MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_237_tokenizer_basic() {
        let t = super::Xm237Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_237_tokenizer_count() {
        let t = super::Xm237Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_237_tokenizer_unique() {
        let t = super::Xm237Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_237_tokenizer_frequency() {
        let t = super::Xm237Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_237_tokenizer_delimiter() {
        let t = super::Xm237Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_237_tokenizer_whitespace() {
        let t = super::Xm237Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_237_tokenizer_empty() {
        let t = super::Xm237Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 237 ----

    #[test]
    fn xn_237_fenwick_prefix_sum() {
        let mut ft = super::Xn237Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_237_fenwick_range_sum() {
        let mut ft = super::Xn237Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_237_fenwick_point_query() {
        let mut ft = super::Xn237Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_237_fenwick_len() {
        let ft = super::Xn237Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_237_fenwick_multiple_updates() {
        let mut ft = super::Xn237Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_237_fenwick_single_element() {
        let mut ft = super::Xn237Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_237_fenwick_find_kth() {
        let mut ft = super::Xn237Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_237_fenwick_negative_delta() {
        let mut ft = super::Xn237Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 237 ----

    #[test]
    fn xn_237_avl_insert_get() {
        let mut m = super::Xn237AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_237_avl_remove() {
        let mut m = super::Xn237AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_237_avl_in_order() {
        let mut m = super::Xn237AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_237_avl_min_max() {
        let mut m = super::Xn237AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_237_avl_floor_ceiling() {
        let mut m = super::Xn237AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_237_avl_height_balanced() {
        let mut m = super::Xn237AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_237_avl_overwrite() {
        let mut m = super::Xn237AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_237_avl_empty() {
        let m: super::Xn237AVL<i32, i32> = super::Xn237AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo237RedBlack tests ---

    #[test]
    fn xo_237_rb_insert_and_get() {
        let mut tree = super::Xo237RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_237_rb_len_and_empty() {
        let mut tree = super::Xo237RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_237_rb_min_max() {
        let mut tree = super::Xo237RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_237_rb_contains() {
        let mut tree = super::Xo237RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_237_rb_remove() {
        let mut tree = super::Xo237RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_237_rb_in_order() {
        let mut tree = super::Xo237RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_237_rb_black_height() {
        let mut tree = super::Xo237RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_237_rb_overwrite() {
        let mut tree = super::Xo237RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo237ConsistentHash tests ---

    #[test]
    fn xo_237_ch_add_and_count() {
        let mut ring = super::Xo237ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_237_ch_remove_node() {
        let mut ring = super::Xo237ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_237_ch_get_node() {
        let mut ring = super::Xo237ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_237_ch_empty_ring() {
        let ring = super::Xo237ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_237_ch_distribution() {
        let mut ring = super::Xo237ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_237_ch_rebalance() {
        let mut ring = super::Xo237ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_237_ch_virtual_nodes() {
        let mut ring = super::Xo237ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_237_ch_consistent_lookup() {
        let mut ring = super::Xo237ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_237_splay_insert_get() {
        let mut t = super::Xp237SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_237_splay_remove() {
        let mut t = super::Xp237SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_237_splay_count_increases() {
        let mut t = super::Xp237SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_237_splay_depth() {
        let mut t = super::Xp237SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_237_splay_len_empty() {
        let t = super::Xp237SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_237_splay_min_max() {
        let mut t = super::Xp237SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_237_splay_overwrite() {
        let mut t = super::Xp237SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_237_splay_remove_missing() {
        let mut t = super::Xp237SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_237 treap tests ----
    #[test]
    fn xq_237_treap_empty() {
        let t = super::Xq237Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_237_treap_insert_get() {
        let mut t = super::Xq237Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_237_treap_overwrite() {
        let mut t = super::Xq237Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_237_treap_remove() {
        let mut t = super::Xq237Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_237_treap_min_max() {
        let mut t = super::Xq237Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_237_treap_rank() {
        let mut t = super::Xq237Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_237_treap_kth() {
        let mut t = super::Xq237Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_237_treap_in_order() {
        let mut t = super::Xq237Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_237 VEB tree tests ----
    #[test]
    fn xq_237_veb_empty() {
        let v = super::Xq237VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_237_veb_insert_contains() {
        let mut v = super::Xq237VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_237_veb_min_max() {
        let mut v = super::Xq237VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_237_veb_delete() {
        let mut v = super::Xq237VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_237_veb_successor() {
        let mut v = super::Xq237VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_237_veb_predecessor() {
        let mut v = super::Xq237VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_237_veb_count() {
        let mut v = super::Xq237VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_237_veb_duplicate_insert() {
        let mut v = super::Xq237VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
