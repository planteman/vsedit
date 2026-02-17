//! Word highlight (same symbol highlighting).
//!
//! Finds and highlights all occurrences of the word under the cursor,
//! distinguishing between read and write references.

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
}
