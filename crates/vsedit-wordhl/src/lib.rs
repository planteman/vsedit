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

impl std::fmt::Display for DocumentHighlightKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
        }
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
        write!(f, "HighlightSet({} items)", self.highlights.len())
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
        assert_eq!(set.to_string(), "HighlightSet(3 items)");
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
}
